//! Fitness-landscape data export for the interactive web visualization.
//!
//! The scoring function [`num_wins`] depends on the 1.2 GB precomputed `hands`
//! table, so the landscape can't be explored in the browser. Instead we run
//! instrumented steepest-ascent climbs here in Rust and emit a small JSON blob
//! embedded into a self-contained HTML page.
//!
//! Each climbing step already enumerates all `C(52,2) = 1326` single-swap
//! neighbors, so one instrumented climb yields the restart trajectory, the
//! neighbor-delta distribution, and the improving-neighbor curve for free.

use crate::cards::Card;
use crate::deck::{generate_adaptive_mutations, AdvancedMutation, Deck};
use crate::game::{hybrid_score, num_wins, position_margin};
use crate::precompute::ScoreTable;
use crate::search::local_search_sa;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

/// We score all 52 rotational cut positions (matches `search::REAL == false`).
const REAL: bool = false;
const MAX_WINS: usize = 52;
const NUM_CARDS: usize = 52;
const NUM_NEIGHBORS: usize = NUM_CARDS * (NUM_CARDS - 1) / 2; // 1326
/// Delta histogram index: bucket `d + DELTA_OFFSET` holds neighbors whose score
/// differs from the current deck by `d` (d ranges -52..=52).
const DELTA_OFFSET: i32 = 52;
const DELTA_BUCKETS: usize = 105; // -52..=52

const HTML_TEMPLATE: &str = include_str!("../viz/template.html");
const DATA_PLACEHOLDER: &str = "/*__LANDSCAPE_DATA__*/";

// ---------------------------------------------------------------------------
// Neighborhood enumeration
// ---------------------------------------------------------------------------

/// Result of enumerating every single-swap neighbor of a deck.
struct Neighborhood {
    /// Best strictly-improving swap, if any.
    best_swap: Option<(usize, usize)>,
    best_score: usize,
    /// Histogram of `neighbor_score - current_score` over all 1326 neighbors.
    hist: [u32; DELTA_BUCKETS],
}

/// Enumerate all `C(52,2)` single-swap neighbors of `deck` (leaves `deck`
/// unchanged) and record the distribution of score deltas.
fn enumerate_neighbors(
    deck: &mut Deck,
    num_players: usize,
    table: &ScoreTable,
    current: usize,
) -> Neighborhood {
    let mut hist = [0u32; DELTA_BUCKETS];
    let mut best_score = current;
    let mut best_swap = None;

    for i in 0..NUM_CARDS {
        for j in (i + 1)..NUM_CARDS {
            deck.0.swap(i, j);
            let s = num_wins(num_players, deck, table, REAL);
            deck.0.swap(i, j); // undo

            let d = s as i32 - current as i32;
            hist[(d + DELTA_OFFSET) as usize] += 1;

            if s > best_score {
                best_score = s;
                best_swap = Some((i, j));
            }
        }
    }

    Neighborhood {
        best_swap,
        best_score,
        hist,
    }
}

/// Split a delta histogram into (worse, equal, better) neighbor counts.
fn counts_from_hist(hist: &[u32; DELTA_BUCKETS]) -> (u32, u32, u32) {
    let equal = hist[DELTA_OFFSET as usize];
    let worse: u32 = hist[..DELTA_OFFSET as usize].iter().sum();
    let better: u32 = hist[DELTA_OFFSET as usize + 1..].iter().sum();
    (worse, equal, better)
}

/// Number of positions at which two decks differ (0..=52).
fn hamming(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

/// Walk from deck `a` to deck `b` one single swap at a time (fixing each
/// position left to right, selection-sort style) and record the score after
/// each swap. The returned sequence includes both endpoints, so `scores[0]` is
/// the score of `a` and the last element is the score of `b`. Every step is a
/// legal move in the swap-neighborhood graph, so this is a real cross-section of
/// the landscape between two decks.
fn swap_path_scores(a: &[u8], b: &[u8], num_players: usize, table: &ScoreTable) -> Vec<usize> {
    let mut deck = Deck(a.iter().map(|&x| Card(x)).collect());
    let mut scores = vec![num_wins(num_players, &deck, table, REAL)];
    for i in 0..NUM_CARDS {
        if deck.0[i].0 == b[i] {
            continue;
        }
        // Find the card b[i] somewhere ahead and swap it into place.
        let mut j = i + 1;
        while j < NUM_CARDS && deck.0[j].0 != b[i] {
            j += 1;
        }
        deck.0.swap(i, j);
        scores.push(num_wins(num_players, &deck, table, REAL));
    }
    scores
}

// ---------------------------------------------------------------------------
// Instrumented climb
// ---------------------------------------------------------------------------

/// Per-step neighbor counts, used to build the improving-neighbor curve.
struct StepCounts {
    score: usize,
    better: u32,
    equal: u32,
    worse: u32,
}

/// The record produced by one random-restart steepest-ascent climb.
struct ClimbRecord {
    /// Score after each step (starts at the random deck, ends at the optimum).
    trajectory: Vec<usize>,
    steps: Vec<StepCounts>,
    /// Delta histogram at the random starting deck.
    start_hist: [u32; DELTA_BUCKETS],
    /// Delta histogram at the local optimum (0 "better" neighbors).
    peak_hist: [u32; DELTA_BUCKETS],
    peak_deck: Vec<u8>,
    peak_score: usize,
}

/// Run one instrumented steepest-ascent climb from a random deck.
fn instrumented_climb(
    num_players: usize,
    table: &ScoreTable,
    rng: &mut oorandom::Rand32,
) -> ClimbRecord {
    let mut deck = Deck::new_deck_order().shuffle(rng);
    let mut score = num_wins(num_players, &deck, table, REAL);

    let mut trajectory = vec![score];
    let mut steps = Vec::new();
    let mut start_hist: Option<[u32; DELTA_BUCKETS]> = None;
    let last_hist;

    loop {
        let nb = enumerate_neighbors(&mut deck, num_players, table, score);
        let (worse, equal, better) = counts_from_hist(&nb.hist);
        steps.push(StepCounts {
            score,
            better,
            equal,
            worse,
        });
        if start_hist.is_none() {
            start_hist = Some(nb.hist);
        }

        match nb.best_swap {
            Some((i, j)) => {
                // Steepest ascent: take the single best-improving swap.
                deck.0.swap(i, j);
                score = nb.best_score;
                trajectory.push(score);
                // Note: we do NOT early-exit at MAX_WINS. The next iteration
                // enumerates the perfect deck, finds no improving neighbor, and
                // breaks — giving us a correct peak_hist for that case too.
            }
            None => {
                // Hill with no steps up: local optimum. This neighborhood
                // (0 improving neighbors) is the peak profile we want.
                last_hist = nb.hist;
                break;
            }
        }
    }

    ClimbRecord {
        trajectory,
        steps,
        start_hist: start_hist.expect("at least one step recorded"),
        peak_hist: last_hist,
        peak_deck: deck.0.iter().map(|c| c.0).collect(),
        peak_score: score,
    }
}

/// Run `restarts` independent climbs for a player count, in parallel.
///
/// Each restart is seeded deterministically from `(seed, num_players, restart)`
/// so the output is reproducible regardless of thread scheduling.
fn run_climbs(
    num_players: usize,
    table: &ScoreTable,
    restarts: usize,
    seed: u64,
) -> Vec<ClimbRecord> {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(restarts.max(1));

    let mut records: Vec<(usize, ClimbRecord)> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for t in 0..num_threads {
            handles.push(scope.spawn(move || {
                let mut out = Vec::new();
                // Round-robin restart indices across threads.
                let mut r = t;
                while r < restarts {
                    let restart_seed = seed
                        .wrapping_mul(1_000_003)
                        ^ ((num_players as u64) << 32)
                        ^ (r as u64);
                    let mut rng = oorandom::Rand32::new(restart_seed);
                    let rec = instrumented_climb(num_players, table, &mut rng);
                    out.push((r, rec));
                    r += num_threads;
                }
                out
            }));
        }
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });

    // Restore deterministic restart order.
    records.sort_by_key(|(r, _)| *r);
    records.into_iter().map(|(_, rec)| rec).collect()
}

// ---------------------------------------------------------------------------
// Serializable DTOs (the JSON contract with the front-end)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Meta {
    max_wins: usize,
    num_cards: usize,
    num_neighbors: usize,
    restarts: usize,
    seed: u64,
    neighborhood: String,
}

#[derive(Serialize)]
struct DeltaBucket {
    delta: i32,
    /// Average neighbor count per deck (summed over restarts / restarts).
    count: f64,
}

#[derive(Serialize)]
struct Profile {
    better: f64,
    equal: f64,
    worse: f64,
    deltas: Vec<DeltaBucket>,
}

#[derive(Serialize)]
struct CurvePoint {
    score: usize,
    better_mean: f64,
    equal_mean: f64,
    worse_mean: f64,
    count: u64,
}

#[derive(Serialize)]
struct DeckView {
    deck: Vec<u8>,
    margins: Vec<i32>,
    score: usize,
}

#[derive(Serialize)]
struct Inspector {
    optimum: DeckView,
    best_found: DeckView,
}

/// A long cross-section that chains together distinct local optima via
/// single-swap paths — a "ridge walk" over the landscape.
#[derive(Serialize)]
struct RidgeWalk {
    scores: Vec<usize>,
    /// Indices into `scores` that sit exactly on a local optimum (the peaks).
    peaks: Vec<usize>,
}

/// One representative climb, decomposed step-by-step so the front-end can draw
/// it as a branching tree: at each visited deck, how many of the 1326 swaps
/// lead up / sideways / down. `better[i]` is the fan-out of uphill options.
#[derive(Serialize)]
struct SampleClimb {
    scores: Vec<usize>,
    better: Vec<u32>,
    equal: Vec<u32>,
    worse: Vec<u32>,
}

/// How one mutation operator behaves: the distribution of fitness changes it
/// produces when applied to decks from a given regime.
#[derive(Serialize)]
struct OpStat {
    name: String,
    desc: String,
    improve: f64, // fraction of applications with delta > 0
    neutral: f64, // delta == 0
    worse: f64,   // delta < 0
    mean_delta: f64,
    median_delta: f64,
    deltas: Vec<DeltaBucket>, // normalized histogram of score deltas
}

/// Operator report card for two regimes: mutating random decks vs. local optima.
#[derive(Serialize)]
struct Operators {
    random: Vec<OpStat>,
    optimum: Vec<OpStat>,
}

/// Per-iteration snapshot of the beam during a beam-search run.
#[derive(Serialize)]
struct BeamIter {
    iter: usize,
    best: usize,
    min: usize,
    median: usize,
    max: usize,
    diversity: f64, // mean pairwise Hamming distance across the beam
}

#[derive(Serialize)]
struct BeamRun {
    iters: Vec<BeamIter>,
    reached: bool,
    beam_width: usize,
}

/// Why the annealer can move where a hill climber can't: at a local optimum,
/// how the 1326 swap-neighbors split when judged by raw win count vs. by the
/// margin-refined (hybrid) score the beam search actually optimizes.
#[derive(Serialize)]
struct MarginGradient {
    wins_up: u32,
    wins_eq: u32,
    wins_dn: u32,
    hyb_up: u32,
    hyb_eq: u32,
    hyb_dn: u32,
}

#[derive(Serialize)]
struct PlayerData {
    n: usize,
    trajectories: Vec<Vec<usize>>,
    peaks: Vec<usize>,
    reached_optimum_count: usize,
    curve: Vec<CurvePoint>,
    profile_random: Profile,
    profile_optimum: Profile,
    inspector: Inspector,
    // topology of the optima
    total_climbs: usize,
    distinct_optima: usize,
    cumulative_distinct: Vec<usize>,
    mean_pairwise_distance: f64,
    ridge: RidgeWalk,
    sample_climb: SampleClimb,
    operators: Operators,
    beam: BeamRun,
    margin_gradient: MarginGradient,
}

fn hamming_cards(a: &[Card], b: &[Card]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

/// At a local optimum, compare the swap-neighborhood judged by raw wins vs. by
/// the margin-refined hybrid score.
fn margin_gradient(base: &[u8], num_players: usize, table: &ScoreTable) -> MarginGradient {
    let mut deck = Deck(base.iter().map(|&x| Card(x)).collect());
    let w0 = num_wins(num_players, &deck, table, REAL);
    let h0 = hybrid_score(num_players, &deck, table, REAL);
    let (mut wu, mut we, mut wd) = (0u32, 0u32, 0u32);
    let (mut hu, mut he, mut hd) = (0u32, 0u32, 0u32);
    for i in 0..NUM_CARDS {
        for j in (i + 1)..NUM_CARDS {
            deck.0.swap(i, j);
            let w = num_wins(num_players, &deck, table, REAL);
            let h = hybrid_score(num_players, &deck, table, REAL);
            deck.0.swap(i, j);
            match w.cmp(&w0) {
                std::cmp::Ordering::Greater => wu += 1,
                std::cmp::Ordering::Equal => we += 1,
                std::cmp::Ordering::Less => wd += 1,
            }
            if h > h0 {
                hu += 1;
            } else if h < h0 {
                hd += 1;
            } else {
                he += 1;
            }
        }
    }
    MarginGradient {
        wins_up: wu,
        wins_eq: we,
        wins_dn: wd,
        hyb_up: hu,
        hyb_eq: he,
        hyb_dn: hd,
    }
}

fn beam_record(beam: &[(Deck, usize, f64)], iter: usize) -> BeamIter {
    let mut wins: Vec<usize> = beam.iter().map(|(_, w, _)| *w).collect();
    wins.sort_unstable();
    let mut sum = 0u64;
    let mut cnt = 0u64;
    for i in 0..beam.len() {
        for j in (i + 1)..beam.len() {
            sum += hamming_cards(&beam[i].0 .0, &beam[j].0 .0) as u64;
            cnt += 1;
        }
    }
    BeamIter {
        iter,
        best: *wins.last().unwrap(),
        min: wins[0],
        median: wins[wins.len() / 2],
        max: *wins.last().unwrap(),
        diversity: if cnt > 0 { sum as f64 / cnt as f64 } else { 0.0 },
    }
}

/// A faithful, instrumented beam search: a diverse population of decks, each
/// round refined by simulated-annealing local search and selected by the
/// margin-refined hybrid score with a diversity guard. Records per-iteration
/// stats and stops when a perfect deck appears (or the iteration cap is hit).
fn instrumented_beam(num_players: usize, table: &ScoreTable, seed: u64) -> BeamRun {
    const W: usize = 40; // beam width
    const M: usize = 8; // mutations spawned per beam member
    const SA: usize = 450; // SA local-search budget per candidate
    const MAX_IT: usize = 160;
    const ELITE: usize = 4;
    const DIV_MIN: usize = 4; // treat neighbors within this Hamming as duplicates

    let mut rng = oorandom::Rand32::new(seed);
    let mut beam: Vec<(Deck, usize, f64)> = (0..W)
        .map(|_| {
            let d = Deck::new_deck_order().shuffle(&mut rng);
            let w = num_wins(num_players, &d, table, REAL);
            let h = hybrid_score(num_players, &d, table, REAL);
            (d, w, h)
        })
        .collect();
    beam.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    let mut iters = vec![beam_record(&beam, 0)];
    let mut reached = beam[0].1 == MAX_WINS;

    for it in 1..=MAX_IT {
        if reached {
            break;
        }
        // Generate candidates in parallel: one thread per beam member.
        let candidates: Vec<(Deck, usize, f64)> = std::thread::scope(|scope| {
            let handles: Vec<_> = beam
                .iter()
                .enumerate()
                .map(|(bi, (bd, _, _))| {
                    let bd = bd.clone();
                    let sd = seed ^ ((it as u64) << 20) ^ (bi as u64).wrapping_mul(0x9E37_79B1);
                    scope.spawn(move || {
                        let mut r = oorandom::Rand32::new(sd);
                        let mut out = Vec::with_capacity(M);
                        for _ in 0..M {
                            let mut child = bd.clone();
                            let k = r.rand_range(1..3);
                            for _ in 0..k {
                                let mu = generate_adaptive_mutations(&mut r, 0.15)
                                    .into_iter()
                                    .next()
                                    .unwrap();
                                child = mu.apply(child, &mut r);
                            }
                            let (opt, w) =
                                local_search_sa(child, num_players, table, SA, 5.0, 0.998, &mut r);
                            let h = hybrid_score(num_players, &opt, table, REAL);
                            out.push((opt, w, h));
                        }
                        out
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });

        // Pool candidates with a few elites, then select the next beam by hybrid
        // score while keeping members mutually diverse.
        let mut pool = candidates;
        for i in 0..ELITE.min(beam.len()) {
            pool.push(beam[i].clone());
        }
        pool.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        let mut nb: Vec<(Deck, usize, f64)> = Vec::with_capacity(W);
        let mut leftover: Vec<(Deck, usize, f64)> = Vec::new();
        for c in pool {
            let diverse = nb
                .iter()
                .all(|(d, _, _)| hamming_cards(&d.0, &c.0 .0) > DIV_MIN);
            if nb.len() < W && diverse {
                nb.push(c);
            } else {
                leftover.push(c);
            }
        }
        for c in leftover {
            if nb.len() >= W {
                break;
            }
            nb.push(c);
        }
        beam = nb;
        beam.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        reached = beam[0].1 == MAX_WINS;
        iters.push(beam_record(&beam, it));
    }

    BeamRun {
        iters,
        reached,
        beam_width: W,
    }
}

/// The GA's five mutation operators (from `deck::AdvancedMutation`), with the
/// same random parameterization `AdvancedMutation::generate` uses.
const OPERATORS: [(&str, &str); 5] = [
    ("Swap", "swap two single cards"),
    ("Block swap", "swap two short runs of cards"),
    ("Reversal", "reverse a run of cards"),
    ("Rotation", "cut the deck at a random point"),
    ("Scramble", "shuffle a short run of cards"),
];

fn gen_operator(kind: usize, rng: &mut oorandom::Rand32) -> AdvancedMutation {
    match kind {
        0 => AdvancedMutation::Swap(
            rng.rand_range(0..52) as usize,
            rng.rand_range(0..52) as usize,
        ),
        1 => {
            let len = rng.rand_range(2..8) as usize;
            let s1 = rng.rand_range(0..(52 - len) as u32) as usize;
            let s2 = rng.rand_range(0..(52 - len) as u32) as usize;
            AdvancedMutation::BlockSwap(s1, s2, len)
        }
        2 => {
            let a = rng.rand_range(0..52) as usize;
            let b = rng.rand_range(0..52) as usize;
            AdvancedMutation::Reversal(a.min(b), a.max(b))
        }
        3 => AdvancedMutation::Rotation(rng.rand_range(1..52) as usize),
        _ => {
            let a = rng.rand_range(0..52) as usize;
            let b = rng.rand_range(0..52) as usize;
            let start = a.min(b);
            let end = a.max(b).min(start + 10);
            AdvancedMutation::Scramble(start, end)
        }
    }
}

/// For each operator, apply it many times to the given base decks and summarize
/// the resulting fitness deltas.
fn operator_stats(
    bases: &[Vec<u8>],
    num_players: usize,
    table: &ScoreTable,
    rng: &mut oorandom::Rand32,
    samples_per_base: usize,
) -> Vec<OpStat> {
    let mut out = Vec::new();
    for (kind, (name, desc)) in OPERATORS.iter().enumerate() {
        let mut hist = [0u64; DELTA_BUCKETS];
        let (mut improve, mut neutral, mut worse, mut total) = (0u64, 0u64, 0u64, 0u64);
        let mut sum_delta = 0i64;
        let mut all_deltas: Vec<i32> = Vec::new();

        for base in bases {
            let deck0 = Deck(base.iter().map(|&x| Card(x)).collect());
            let s0 = num_wins(num_players, &deck0, table, REAL) as i32;
            for _ in 0..samples_per_base {
                let op = gen_operator(kind, rng);
                let mutated = op.apply(deck0.clone(), rng);
                let d = num_wins(num_players, &mutated, table, REAL) as i32 - s0;
                hist[(d.clamp(-52, 52) + DELTA_OFFSET) as usize] += 1;
                match d.cmp(&0) {
                    std::cmp::Ordering::Greater => improve += 1,
                    std::cmp::Ordering::Equal => neutral += 1,
                    std::cmp::Ordering::Less => worse += 1,
                }
                sum_delta += d as i64;
                total += 1;
                all_deltas.push(d);
            }
        }

        all_deltas.sort_unstable();
        let median = if all_deltas.is_empty() {
            0.0
        } else {
            all_deltas[all_deltas.len() / 2] as f64
        };
        let denom = total.max(1) as f64;
        let deltas: Vec<DeltaBucket> = hist
            .iter()
            .enumerate()
            .filter_map(|(i, &c)| {
                if c == 0 {
                    return None;
                }
                Some(DeltaBucket {
                    delta: i as i32 - DELTA_OFFSET,
                    count: c as f64 / denom,
                })
            })
            .collect();

        out.push(OpStat {
            name: name.to_string(),
            desc: desc.to_string(),
            improve: improve as f64 / denom,
            neutral: neutral as f64 / denom,
            worse: worse as f64 / denom,
            mean_delta: sum_delta as f64 / denom,
            median_delta: median,
            deltas,
        });
    }
    out
}

#[derive(Serialize)]
struct Landscape {
    meta: Meta,
    players: Vec<PlayerData>,
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// Convert a summed delta histogram into an averaged, sparse profile.
fn profile_from_hist(hist: &[u64; DELTA_BUCKETS], restarts: usize) -> Profile {
    let denom = restarts.max(1) as f64;
    let mut deltas = Vec::new();
    let mut better = 0.0;
    let mut equal = 0.0;
    let mut worse = 0.0;

    for (idx, &sum) in hist.iter().enumerate() {
        if sum == 0 {
            continue;
        }
        let delta = idx as i32 - DELTA_OFFSET;
        let avg = sum as f64 / denom;
        deltas.push(DeltaBucket { delta, count: avg });
        match delta.cmp(&0) {
            std::cmp::Ordering::Greater => better += avg,
            std::cmp::Ordering::Equal => equal += avg,
            std::cmp::Ordering::Less => worse += avg,
        }
    }

    Profile {
        better,
        equal,
        worse,
        deltas,
    }
}

/// Reconstruct a [`Deck`] from raw card bytes and compute per-cut margins.
fn deck_view(cards: &[u8], num_players: usize, table: &ScoreTable) -> DeckView {
    let deck = Deck(cards.iter().map(|&b| Card(b)).collect());
    let margins: Vec<i32> = (0..NUM_CARDS)
        .map(|pos| position_margin(num_players, &deck, pos, table))
        .collect();
    let score = num_wins(num_players, &deck, table, REAL);
    DeckView {
        deck: cards.to_vec(),
        margins,
        score,
    }
}

fn build_player_data(
    num_players: usize,
    table: &ScoreTable,
    restarts: usize,
    seed: u64,
) -> PlayerData {
    eprintln!(
        "  ⛰️  n={}: running {} instrumented climbs...",
        num_players, restarts
    );
    let records = run_climbs(num_players, table, restarts, seed);

    let mut trajectories = Vec::with_capacity(records.len());
    let mut peaks = Vec::with_capacity(records.len());
    let mut reached_optimum_count = 0;

    // Improving-neighbor curve: sum counts keyed by current score.
    let mut curve_acc: BTreeMap<usize, (u64, u64, u64, u64)> = BTreeMap::new();

    let mut random_hist = [0u64; DELTA_BUCKETS];
    let mut optimum_hist = [0u64; DELTA_BUCKETS];

    let mut example_optimum: Option<Vec<u8>> = None;
    let mut best_deck: Vec<u8> = Vec::new();
    let mut best_score = 0usize;
    let mut all_peaks: Vec<Vec<u8>> = Vec::with_capacity(records.len());

    for rec in &records {
        trajectories.push(rec.trajectory.clone());
        peaks.push(rec.peak_score);
        all_peaks.push(rec.peak_deck.clone());
        if rec.peak_score == MAX_WINS {
            reached_optimum_count += 1;
        }

        for st in &rec.steps {
            let e = curve_acc.entry(st.score).or_insert((0, 0, 0, 0));
            e.0 += st.better as u64;
            e.1 += st.equal as u64;
            e.2 += st.worse as u64;
            e.3 += 1;
        }

        for k in 0..DELTA_BUCKETS {
            random_hist[k] += rec.start_hist[k] as u64;
            optimum_hist[k] += rec.peak_hist[k] as u64;
        }

        if example_optimum.is_none() {
            example_optimum = Some(rec.peak_deck.clone());
        }
        if rec.peak_score > best_score {
            best_score = rec.peak_score;
            best_deck = rec.peak_deck.clone();
        }
    }

    let curve: Vec<CurvePoint> = curve_acc
        .into_iter()
        .map(|(score, (b, e, w, count))| {
            let denom = count.max(1) as f64;
            CurvePoint {
                score,
                better_mean: b as f64 / denom,
                equal_mean: e as f64 / denom,
                worse_mean: w as f64 / denom,
                count,
            }
        })
        .collect();

    let profile_random = profile_from_hist(&random_hist, records.len());
    let profile_optimum = profile_from_hist(&optimum_hist, records.len());

    let example_optimum = example_optimum.unwrap_or_else(|| best_deck.clone());
    let inspector = Inspector {
        optimum: deck_view(&example_optimum, num_players, table),
        best_found: deck_view(&best_deck, num_players, table),
    };

    // ---- topology of the local optima ----
    // Distinct optima (exact) in discovery order, plus a running distinct count.
    let mut distinct: Vec<Vec<u8>> = Vec::new();
    let mut cumulative_distinct = Vec::with_capacity(all_peaks.len());
    for pk in &all_peaks {
        if !distinct.iter().any(|d| d == pk) {
            distinct.push(pk.clone());
        }
        cumulative_distinct.push(distinct.len());
    }

    // Mean pairwise Hamming distance between distinct optima (how spread out).
    let mut dsum = 0u64;
    let mut dcnt = 0u64;
    for i in 0..distinct.len() {
        for j in (i + 1)..distinct.len() {
            dsum += hamming(&distinct[i], &distinct[j]) as u64;
            dcnt += 1;
        }
    }
    let mean_pairwise_distance = if dcnt > 0 { dsum as f64 / dcnt as f64 } else { 0.0 };

    // Ridge walk: chain up to 10 distinct optima with single-swap paths, so the
    // cross-section rises to each summit and dips into the valley between them.
    let r = distinct.len().min(10);
    let mut ridge_scores: Vec<usize> = Vec::new();
    let mut ridge_peaks: Vec<usize> = Vec::new();
    if r >= 1 {
        ridge_scores.push(num_wins(
            num_players,
            &Deck(distinct[0].iter().map(|&x| Card(x)).collect()),
            table,
            REAL,
        ));
        ridge_peaks.push(0);
        for k in 1..r {
            // swap_path_scores includes both endpoints; skip the first (it equals
            // the previous peak we already recorded) to keep the walk continuous.
            let seg = swap_path_scores(&distinct[k - 1], &distinct[k], num_players, table);
            ridge_scores.extend(seg.into_iter().skip(1));
            ridge_peaks.push(ridge_scores.len() - 1);
        }
    }
    let ridge = RidgeWalk {
        scores: ridge_scores,
        peaks: ridge_peaks,
    };

    // Mutation-operator report card: how each GA operator changes fitness, both
    // for random decks and near local optima.
    let mut op_rng = oorandom::Rand32::new(
        seed.wrapping_mul(2_654_435_761).wrapping_add(num_players as u64),
    );
    const N_BASES: usize = 40;
    let random_bases: Vec<Vec<u8>> = (0..N_BASES)
        .map(|_| {
            Deck::new_deck_order()
                .shuffle(&mut op_rng)
                .0
                .iter()
                .map(|c| c.0)
                .collect()
        })
        .collect();
    let opt_bases: Vec<Vec<u8>> = (0..N_BASES)
        .map(|k| distinct[k % distinct.len()].clone())
        .collect();
    let operators = Operators {
        random: operator_stats(&random_bases, num_players, table, &mut op_rng, 25),
        optimum: operator_stats(&opt_bases, num_players, table, &mut op_rng, 25),
    };

    // Why beam search escapes: the margin gradient at a local optimum, and an
    // instrumented beam-search run that breaks through the ceiling.
    let margin_gradient = margin_gradient(&distinct[0], num_players, table);
    let beam = instrumented_beam(num_players, table, seed.wrapping_add(97 + num_players as u64));

    // A representative climb (median trajectory length) to draw as a tree.
    let mut order: Vec<usize> = (0..records.len()).collect();
    order.sort_by_key(|&i| records[i].trajectory.len());
    let rep = &records[order[order.len() / 2]];
    let sample_climb = SampleClimb {
        scores: rep.trajectory.clone(),
        better: rep.steps.iter().map(|s| s.better).collect(),
        equal: rep.steps.iter().map(|s| s.equal).collect(),
        worse: rep.steps.iter().map(|s| s.worse).collect(),
    };

    let best_peak = peaks.iter().copied().max().unwrap_or(0);
    eprintln!(
        "  ✓ n={}: best peak {}/{}, reached 52 in {}/{} restarts · {} distinct optima, ~{:.0}/{} apart",
        num_players, best_peak, MAX_WINS, reached_optimum_count, restarts,
        distinct.len(), mean_pairwise_distance, NUM_CARDS
    );

    PlayerData {
        n: num_players,
        trajectories,
        peaks,
        reached_optimum_count,
        curve,
        profile_random,
        profile_optimum,
        inspector,
        total_climbs: records.len(),
        distinct_optima: distinct.len(),
        cumulative_distinct,
        mean_pairwise_distance,
        ridge,
        sample_climb,
        operators,
        beam,
        margin_gradient,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the landscape experiments and write a self-contained HTML file.
pub fn export(
    table: &ScoreTable,
    players: &[usize],
    restarts: usize,
    seed: u64,
    out_path: &Path,
) -> std::io::Result<()> {
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Fitness-landscape visualization export");
    eprintln!(
        "  players: {:?}, restarts each: {}, seed: {}",
        players, restarts, seed
    );
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!();

    let player_data: Vec<PlayerData> = players
        .iter()
        .map(|&n| build_player_data(n, table, restarts, seed))
        .collect();

    let landscape = Landscape {
        meta: Meta {
            max_wins: MAX_WINS,
            num_cards: NUM_CARDS,
            num_neighbors: NUM_NEIGHBORS,
            restarts,
            seed,
            neighborhood: format!("single swap, C(52,2) = {} neighbors", NUM_NEIGHBORS),
        },
        players: player_data,
    };

    let json = serde_json::to_string(&landscape).expect("serialize landscape");
    let data_stmt = format!("const DATA = {};", json);
    let html = HTML_TEMPLATE.replace(DATA_PLACEHOLDER, &data_stmt);

    let mut file = std::fs::File::create(out_path)?;
    file.write_all(html.as_bytes())?;

    eprintln!();
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!(
        "  ✓ Wrote {} ({} bytes)",
        out_path.display(),
        html.len()
    );
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

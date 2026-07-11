use clap::Parser;
use z3::ast::{Ast, Bool, Int};
use z3::{Config, Context, SatResult, Solver};

#[derive(Parser, Debug)]
#[command(name = "poker_smt")]
#[command(about = "SMT-based solver for optimal poker deck ordering")]
struct Args {
    /// Number of players (including dealer at position 0)
    #[arg(short, long, default_value = "2")]
    num_players: usize,

    /// Timeout in seconds (0 for no timeout)
    #[arg(short, long, default_value = "3600")]
    timeout: u64,

    /// Dump the generated SMT-LIB2 formula to this file and exit without solving
    #[arg(short, long)]
    dump: Option<String>,
}

// Card encoding: card_id = suit * 13 + value
// suit: 0=clubs, 1=diamonds, 2=hearts, 3=spades
// value: 0=2, 1=3, ..., 9=J, 10=Q, 11=K, 12=A

fn card_to_string(card_id: i64) -> String {
    let suit = card_id / 13;
    let value = card_id % 13;

    let suit_char = match suit {
        0 => 'c',
        1 => 'd',
        2 => 'h',
        3 => 's',
        _ => '?',
    };

    let value_str = match value {
        0..=8 => format!("{}", value + 2),
        9 => "J".to_string(),
        10 => "Q".to_string(),
        11 => "K".to_string(),
        12 => "A".to_string(),
        _ => "?".to_string(),
    };

    format!("{}{}", value_str, suit_char)
}

// Helper to create suit extraction: card / 13
fn suit<'ctx>(ctx: &'ctx Context, card: &Int<'ctx>) -> Int<'ctx> {
    card / Int::from_i64(ctx, 13)
}

// Helper to create value extraction: card % 13
fn value<'ctx>(ctx: &'ctx Context, card: &Int<'ctx>) -> Int<'ctx> {
    card.modulo(&Int::from_i64(ctx, 13))
}

// Check if all 5 cards have the same suit
fn is_flush<'ctx>(ctx: &'ctx Context, cards: &[Int<'ctx>; 5]) -> Bool<'ctx> {
    let s0 = suit(ctx, &cards[0]);
    let s1 = suit(ctx, &cards[1]);
    let s2 = suit(ctx, &cards[2]);
    let s3 = suit(ctx, &cards[3]);
    let s4 = suit(ctx, &cards[4]);

    Bool::and(ctx, &[
        &s0._eq(&s1),
        &s1._eq(&s2),
        &s2._eq(&s3),
        &s3._eq(&s4),
    ])
}

// Sort 5 values ascending with a fixed 9-comparator sorting network (optimal
// for n=5 elements). Everything downstream (straight detection, pair
// pattern, high card) reads off this sorted order instead of re-deriving it
// with per-value existential searches.
fn sort5<'ctx>(values: [Int<'ctx>; 5]) -> [Int<'ctx>; 5] {
    let mut v = values;
    const NETWORK: [(usize, usize); 9] =
        [(0, 1), (3, 4), (2, 4), (2, 3), (0, 3), (0, 2), (1, 4), (1, 3), (1, 2)];
    for &(i, j) in &NETWORK {
        let le = v[i].le(&v[j]);
        let lo = le.ite(&v[i], &v[j]);
        let hi = le.ite(&v[j], &v[i]);
        v[i] = lo;
        v[j] = hi;
    }
    v
}

// Determine hand rank + high-card tiebreaker for 5 cards, matching the
// simplified (rank, high_card) scoring the rest of this project uses (see
// ../src/hands.rs::score_five_cards) so a solution found here means the same
// thing as a win anywhere else in the codebase. Returned as a single
// comparable integer: rank * 100 + high_card.
//
// Once the 5 values are sorted, equal values are necessarily contiguous, so
// the entire pair/two-pair/trips/full-house/quad pattern is determined by
// just the 4 adjacent-equality booleans — no per-value counting sweep
// needed. Straight and flush can only occur when all 5 values are distinct
// (a repeated value rules out 5 consecutive values, and a repeated suit
// would require a repeated (suit, value) card, which the deck's global
// distinctness constraint rules out), so the pair-pattern ranks and the
// straight/flush ranks never collide and can be layered without the
// "and not X" guards the old code needed to avoid clobbering straight
// flushes.
fn hand_score<'ctx>(ctx: &'ctx Context, cards: &[Int<'ctx>; 5]) -> Int<'ctx> {
    let raw_values: [Int; 5] = std::array::from_fn(|i| value(ctx, &cards[i]));
    let v = sort5(raw_values);
    let is_flush_val = is_flush(ctx, cards);

    let e01 = v[0]._eq(&v[1]);
    let e12 = v[1]._eq(&v[2]);
    let e23 = v[2]._eq(&v[3]);
    let e34 = v[3]._eq(&v[4]);

    let quad = Bool::or(ctx, &[
        &Bool::and(ctx, &[&e12, &e23, &e34, &e01.not()]),
        &Bool::and(ctx, &[&e01, &e12, &e23, &e34.not()]),
    ]);
    let full_house = Bool::or(ctx, &[
        &Bool::and(ctx, &[&e01, &e23, &e34, &e12.not()]),
        &Bool::and(ctx, &[&e01, &e12, &e34, &e23.not()]),
    ]);
    let trips = Bool::or(ctx, &[
        &Bool::and(ctx, &[&e01, &e12, &e23.not(), &e34.not()]),
        &Bool::and(ctx, &[&e12, &e23, &e01.not(), &e34.not()]),
        &Bool::and(ctx, &[&e23, &e34, &e01.not(), &e12.not()]),
    ]);
    let two_pair = Bool::or(ctx, &[
        &Bool::and(ctx, &[&e01, &e23, &e12.not(), &e34.not()]),
        &Bool::and(ctx, &[&e01, &e34, &e12.not(), &e23.not()]),
        &Bool::and(ctx, &[&e12, &e34, &e01.not(), &e23.not()]),
    ]);
    let one_pair = Bool::or(ctx, &[
        &Bool::and(ctx, &[&e01, &e12.not(), &e23.not(), &e34.not()]),
        &Bool::and(ctx, &[&e12, &e01.not(), &e23.not(), &e34.not()]),
        &Bool::and(ctx, &[&e23, &e01.not(), &e12.not(), &e34.not()]),
        &Bool::and(ctx, &[&e34, &e01.not(), &e12.not(), &e23.not()]),
    ]);

    let is_wheel = Bool::and(ctx, &[
        &v[0]._eq(&Int::from_i64(ctx, 0)),
        &v[1]._eq(&Int::from_i64(ctx, 1)),
        &v[2]._eq(&Int::from_i64(ctx, 2)),
        &v[3]._eq(&Int::from_i64(ctx, 3)),
        &v[4]._eq(&Int::from_i64(ctx, 12)),
    ]);
    let is_straight_val = Bool::or(ctx, &[
        &is_wheel,
        &Bool::and(ctx, &[
            &v[1]._eq(&(v[0].clone() + Int::from_i64(ctx, 1))),
            &v[2]._eq(&(v[1].clone() + Int::from_i64(ctx, 1))),
            &v[3]._eq(&(v[2].clone() + Int::from_i64(ctx, 1))),
            &v[4]._eq(&(v[3].clone() + Int::from_i64(ctx, 1))),
        ]),
    ]);

    let mut rank = Int::from_i64(ctx, 0); // default: high card
    rank = one_pair.ite(&Int::from_i64(ctx, 1), &rank);
    rank = two_pair.ite(&Int::from_i64(ctx, 2), &rank);
    rank = trips.ite(&Int::from_i64(ctx, 3), &rank);
    rank = full_house.ite(&Int::from_i64(ctx, 6), &rank);
    rank = quad.ite(&Int::from_i64(ctx, 7), &rank);
    rank = is_straight_val.ite(&Int::from_i64(ctx, 4), &rank);
    rank = is_flush_val.ite(&Int::from_i64(ctx, 5), &rank);
    rank = Bool::and(ctx, &[&is_straight_val, &is_flush_val]).ite(&Int::from_i64(ctx, 8), &rank);

    // High card tiebreaker, matching hands.rs::score_five_cards exactly: an
    // ace counts as 14 unless it's completing the wheel, where the "5"
    // (v[3], since the ace sorts to the top as value 12 in this encoding)
    // is the effective high card.
    let has_ace = v[4]._eq(&Int::from_i64(ctx, 12));
    let hi = is_wheel.ite(
        &Int::from_i64(ctx, 5),
        &has_ace.ite(&Int::from_i64(ctx, 14), &(v[4].clone() + Int::from_i64(ctx, 2))),
    );

    rank * Int::from_i64(ctx, 100) + hi
}

// Get the best hand from 7 cards (2 hole + 5 community): the max hand_score
// over all C(7,5) = 21 combinations. Because hand_score is already a single
// comparable integer, the running-best update is one comparison instead of
// a 6-field (rank + 5 tiebreakers) lexicographic compare.
fn best_hand_from_seven<'ctx>(
    ctx: &'ctx Context,
    hole_cards: &[Int<'ctx>; 2],
    community: &[Int<'ctx>; 5],
) -> Int<'ctx> {
    // All C(7,5) = 21 combinations
    let all_cards = [
        hole_cards[0].clone(),
        hole_cards[1].clone(),
        community[0].clone(),
        community[1].clone(),
        community[2].clone(),
        community[3].clone(),
        community[4].clone(),
    ];

    let combinations: [[usize; 5]; 21] = [
        [0,1,2,3,4], [0,1,2,3,5], [0,1,2,3,6], [0,1,2,4,5], [0,1,2,4,6],
        [0,1,2,5,6], [0,1,3,4,5], [0,1,3,4,6], [0,1,3,5,6], [0,1,4,5,6],
        [0,2,3,4,5], [0,2,3,4,6], [0,2,3,5,6], [0,2,4,5,6], [0,3,4,5,6],
        [1,2,3,4,5], [1,2,3,4,6], [1,2,3,5,6], [1,2,4,5,6], [1,3,4,5,6],
        [2,3,4,5,6],
    ];

    let mut best = Int::from_i64(ctx, -1);
    for combo in combinations {
        let hand = [
            all_cards[combo[0]].clone(),
            all_cards[combo[1]].clone(),
            all_cards[combo[2]].clone(),
            all_cards[combo[3]].clone(),
            all_cards[combo[4]].clone(),
        ];
        let score = hand_score(ctx, &hand);
        best = score.gt(&best).ite(&score, &best);
    }
    best
}

fn main() {
    let args = Args::parse();

    println!("Poker SMT Solver");
    println!("Players: {}", args.num_players);
    println!("Timeout: {} seconds", args.timeout);
    println!();

    let cfg = Config::new();
    let ctx = Context::new(&cfg);
    let solver = Solver::new(&ctx);

    // Set timeout
    if args.timeout > 0 {
        let mut params = z3::Params::new(&ctx);
        params.set_u32("timeout", (args.timeout * 1000) as u32);
        solver.set_params(&params);
    }

    println!("Creating deck variables (52 cards)...");

    // Create 52 integer variables for the deck
    let deck: Vec<Int> = (0..52)
        .map(|i| Int::new_const(&ctx, format!("card_{}", i)))
        .collect();

    // Constraint: each card is in range [0, 51]
    for card in &deck {
        solver.assert(&card.ge(&Int::from_i64(&ctx, 0)));
        solver.assert(&card.le(&Int::from_i64(&ctx, 51)));
    }

    println!("Adding permutation constraint...");

    // Constraint: all cards are distinct (valid permutation)
    solver.assert(&Int::distinct(&ctx, &deck.iter().collect::<Vec<_>>()));

    println!("Generating constraints for all 52 cut positions...");
    println!("(This will take a while - generating thousands of constraints)");
    println!();

    // For each cut position
    for cut in 0..52 {
        if cut % 10 == 0 {
            println!("  Processing cut position {}/52...", cut);
        }

        // Create the cut deck: [cut, cut+1, ..., 51, 0, 1, ..., cut-1]
        let cut_deck: Vec<Int> = (0..52)
            .map(|i| deck[(cut + i) % 52].clone())
            .collect();

        // Deal cards to players
        let n = args.num_players;
        let mut player_hands: Vec<[Int; 2]> = Vec::new();

        for p in 0..n {
            player_hands.push([
                cut_deck[2 * p].clone(),
                cut_deck[2 * p + 1].clone(),
            ]);
        }

        // Community cards
        let community = [
            cut_deck[2 * n].clone(),
            cut_deck[2 * n + 1].clone(),
            cut_deck[2 * n + 2].clone(),
            cut_deck[2 * n + 3].clone(),
            cut_deck[2 * n + 4].clone(),
        ];

        // Get best hand score for each player
        let player_best_hands: Vec<Int> = (0..n)
            .map(|p| best_hand_from_seven(&ctx, &player_hands[p], &community))
            .collect();

        // Constraint: player 0 must win (beat all other players)
        for p in 1..n {
            solver.assert(&player_best_hands[0].gt(&player_best_hands[p]));
        }
    }

    println!();
    println!("All constraints generated!");

    // If requested, dump the SMT-LIB2 formula to disk and exit without solving.
    if let Some(path) = &args.dump {
        let formula = solver.to_string();
        std::fs::write(path, &formula)
            .unwrap_or_else(|e| panic!("Failed to write formula to {}: {}", path, e));
        println!(
            "Formula written to {} ({} bytes, {} assertions)",
            path,
            formula.len(),
            formula.matches("(assert").count()
        );
        return;
    }

    println!("Starting solver (this may take a very long time)...");
    println!();

    match solver.check() {
        SatResult::Sat => {
            println!("SAT! Found a solution!");
            println!();

            let model = solver.get_model().unwrap();

            println!("Winning deck ordering:");
            for i in 0..52 {
                let card_val = model.eval(&deck[i], true).unwrap().as_i64().unwrap();
                println!("  Position {}: {}", i, card_to_string(card_val));
            }

            println!();
            println!("Deck as comma-separated card IDs:");
            let card_ids: Vec<String> = (0..52)
                .map(|i| model.eval(&deck[i], true).unwrap().as_i64().unwrap().to_string())
                .collect();
            println!("{}", card_ids.join(","));
        }
        SatResult::Unsat => {
            println!("UNSAT: No deck ordering exists where player 0 wins all {} positions.", 52);
        }
        SatResult::Unknown => {
            println!("UNKNOWN: Solver could not determine satisfiability (likely timeout).");
            println!("Try increasing the timeout or simplifying the problem.");
        }
    }
}

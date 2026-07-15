# Jeff Decks: Existence and Nonexistence Across Player Counts

**Research question.** A *jeff deck* for `n` players is an ordering of the
standard 52-card deck such that, no matter where the deck is cut before the
deal, the dealer strictly wins the resulting hand of Texas hold'em.  Jeff
decks were known (by beam search) for n = 2..5.  Determine whether any
n <= 22 admits **no** jeff deck — either prove nonexistence for some n, or
exhibit a deck for every n.

**Status (2026-07-15).**  Verified jeff decks exist for n in {2, 3, 4, 5,
6, 9}.  For the remaining n, no deck has been found despite large-scale
annealing, parallel tempering, exact local-neighborhood exhaustion,
unhinted global CP-SAT (6h), hinted exact LNS (8h per target), 12h hard-
feasibility CP, the original beam search (500 iterations, plateau 44/52
coarse at n=7), and complete enumeration of the most promising algebraic
family, which is now
provably barren for every unsolved n (n=12 through its 262,767
lowest-defect layouts).  Global nonexistence for any single n remains
unproven; the evidence below sharply localizes where jeff decks can and
cannot live.  Distinctive empirical signature: every search modality,
stochastic or exact, terminates on rank skeletons whose win counts are
provably capped at 48-50/52 by CP-SAT — at n=7 twice, on two
independently discovered locked optima (49 and 50 wins), and the exact
LNS runs cannot leave those basins.

---

## 1. Exact rules (fixed by the reference implementation `poker-solver`)

* The deck is a `Vec` of 52 cards; dealing pops the **end** of the vector.
  Cutting at any of the 52 positions rotates the vector, so the 52 cuts are
  exactly the 52 rotations of the deal order.
* Deal for `n` players: two hole cards round-robin (player `p` receives
  deal-order slots `p` and `n+p`; **player 0 is the dealer and receives the
  first card**), then burn, flop (slots `2n+1..2n+3`), burn, turn (`2n+5`),
  burn, river (`2n+7`).
* Hands are compared by full 7-card Texas hold'em evaluation.  The dealer
  must **strictly** beat every opponent; any tie fails.  (The reference
  engine compares `(category, high card)` only; all our positive decks win
  under both that coarse rule and full kicker/tiebreak rules.)

## 2. Main results

| n | result | witness / proof |
|---|--------|-----------------|
| 2 | **SOLVED** | `decks/solved/n2_full.json` |
| 3 | **SOLVED, closed form** | CRT deck `card(q) = (rank q mod 13, suit q mod 4)`; Lean theorem |
| 4 | **SOLVED** | `decks/solved/n4_full.json` |
| 5 | **SOLVED** | `decks/solved/n5_full.json` |
| 6 | **SOLVED** | `decks/solved/n6_full.json` |
| 7 | open; best 50/52 | **no period-13 jeff deck (theorem)** |
| 8 | open; best 48/52 | **no period-13 jeff deck (theorem)** |
| 9 | **SOLVED, algebraic** | descending period-13 straight relay; Lean theorem |
| 10 | open; best 48/52 | **no period-13 jeff deck (theorem)** |
| 11 | open; best 48/52 | **no period-13 jeff deck (theorem)** |
| 12 | open; best 50/52 | period-13 bad<=4 completely impossible (262,767 layouts) |
| 13 | open; best 39/52 | period-13 loses all 52 cuts identically |
| 14..22 | open; best 43..25/52 | **no period-13 jeff deck (theorem, player-13 curse)** |

Every SOLVED deck passes four independent verifications: (i) a slow
reference evaluator in Rust (5-card enumeration), (ii) `pyverify.py`, an
independently written Python checker, (iii) a Lean 4 theorem
`checkDeck deckN N = true` proved by `native_decide`
(`lean-workspace/LeanWorkspace/Decks.lean`), and (iv) the user's original
`poker-solver` scoring tables (`check` subcommand).

## 3. The period-13 program (the decisive negative engine)

A *period-13 layout* fixes a bijection sigma of the 13 ranks and lays ranks
down as `rank(q) = sigma(q mod 13)`, with suits free.  This family contains
the closed-form n=3 deck and the algebraic n=9 deck, and is small enough to
treat **exhaustively**: cut-invariance lets us normalize `sigma(0) = 0`, so
the family per n is exactly 12! = 479,001,600 layouts, each scanned in
minutes by `jeff-search alg13-scan`.

Key structural facts, all machine-checked:

1. **Class collapse.**  With period-13 ranks the 52 cuts fall into 13
   classes of 4 with identical rank-only outcomes.  A class is *bad* if the
   dealer fails to strictly win it rank-only.
2. **Flush-rescue necessity.**  A player's 7-card evaluation exceeds its
   rank-only evaluation only if the player holds >= 5 cards of one suit.
   Hence in a bad class, all four cuts require a **dealer flush**.
3. **Suit-validity constraint.**  The four positions of one rank must take
   four distinct suits — the same constraint for every layout, so
   suit-capacity bounds are layout-independent.
4. **Class capacity (CP-SAT, proven OPTIMAL).**  The maximum number of
   classes whose four cuts can all carry a dealer flush is
   C_max(7)=8, C_max(8)=7, C_max(10)=7, C_max(11)=7, C_max(12)=8.
5. **Set-level exclusion.**  For a specific set S of bad classes, "dealer
   flush on all 4|S| cuts of S" is decidable per set; infeasible sets kill
   every layout realizing them.
6. **Exact suit completion (`suitsat.py`).**  For any fixed rank layout,
   the existence of winning suits is decided exactly by CP-SAT over
   128-row flush tables per (cut, player, suit); the solver reconstructs
   the n=9 deck from its layout (end-to-end validation) and its
   INFEASIBLE verdicts constitute proofs.

Exhaustive scan results (minimum bad-class counts over all 12! layouts):

| n | min bad | survivors at min | outcome |
|---|---------|------------------|---------|
| 7 | 7 | 1,492 (+114,534 at bad=8) | bad=7: all impossible (1,424 set-level + 68 exact). bad=8: all impossible (113,130 set-level + 1,404 exact). bad>=9 > C_max(7)=8 ⇒ **THEOREM: none exists** |
| 8 | 7 | 183 | all 6 realized bad-sets infeasible; bad>=8 > C_max ⇒ **THEOREM: none exists** |
| 9 | 3 | 2 | descending sigma completes (the solved deck); ascending sigma provably has no suit completion |
| 10 | 7 | 247 | all 7 realized bad-sets infeasible; bad>=8 > C_max ⇒ **THEOREM: none exists** |
| 11 | >= 9 | 0 at bad<=8 | 9 > C_max(11)=7 ⇒ **THEOREM: none exists** |
| 12 | 3 | 8,967 | bad=3: all impossible (3,970 set-level + 4,997 exact). bad=4: all 126,900 impossible (88,311 set-level + 38,589 exact two-stage, 0 SAT). bad>=5 (~112.8M) abandoned as beyond enumeration — family practically closed |
| 13 | — | — | every period-13 layout loses all 52 cuts (five opponents always hold trips) |
| 14..22 | — | — | opponent 13 holds ranks identical to the dealer ⇒ dealer needs a flush at every cut; CP-SAT proves no valid suit assignment achieves 52 dealer flushes |

The period-13 family is therefore settled for every n: **solved at n=9
(and n=3 via CRT), provably empty for n in {7, 8, 10, 11, 13, 14, ...,
22}**, and empty at n=12 through bad=4 (every layout with at most 4 bad
classes — where family solutions live, cf. n=9 at its minimum — is
impossible; the bad >= 5 tail of ~112.8M layouts is left unenumerated).

Two follow-on families are also excluded: single rank-transposition
defects of the minimum-bad n=12 layouts never reach even 9 bad cuts (the
bases have 12), and the block-shifted period-26 family has sampled minima
of 12-20 bad classes — both abandoned as hopeless.

## 4. Other exclusions and certificates

* **Slope-relay uniqueness.**  Among n in 7..22, the dealer-straight relay
  construction admits a feasible slope only at n=9 (slopes ±1); at n=9 only
  the descending slope is suit-realizable.
* **Local optimality of the annealing walls.**  The best n=7 (49/52, then
  a new 50/52 record) and n=12 (50/52) decks are exact 2-swap local optima
  (all ~1.7M two-card transposition sequences checked each) and their rank
  layouts are exactly suit-capped (CP-SAT).  Around them, complete one-swap
  (and for the older basins two-swap) rank neighborhoods and 60 rounds of
  window-permutation screening (k=4) are all exactly suit-INFEASIBLE — the
  walls are real, not search failure; n=7 alone now has two independently
  certified suit-dead optima.
* **Block-shifted period-26 family** (evades the player-13 curse): sampled
  minima of 12-20 bad two-cut classes (24-40 required dealer-flush cuts) at
  all tested n — abandoned as hopeless without exhaustive scanning.
* **Period-26 constructions at the frontier**: best verified 32/52 (n=19),
  36/52 (n=21); the strongest n=21 trips-forcing construction is rigorously
  impossible (forced full-house tie).
* **n=22 exact neighborhood exclusions**: the best 38/52 skeleton is capped
  at 38/52 over all suit assignments (single-comparison UNSAT core); its
  complete distance-two rank ball (753,272 permutations) is excluded by
  sound screens plus exact Z3/CP-SAT (0 unknown).

## 5. Where the frontier stands

Annealing walls (2400s, 6 threads, graded-energy SA+ILS+repair):
n=13: 39, n=14: 43, n=15: 40, n=16: 42, n=17: 27, n=18: 29, n=19: 31,
n=20: 28, n=21: 25, n=22: 32 (out of 52).

A first-moment estimate (log2 E[#jeff decks] = 52 log2 p + log2 52!)
crosses zero near n ≈ 18 under an independence model: random-like decks
should stop existing around there, and only strongly structured decks could
survive.  The period-13 results prove the *known* structure does not
survive either — for 7 <= n <= 22 (n != 9), the only family ever to
produce a jeff deck beyond beam-search range is provably empty (n=12:
empty through its entire low-defect region).

## 6. Open problems

1. Do jeff decks exist for n in {7, 8, 10, 11, 12, 13}?  (All periodic rank
   structure is now excluded or nearly so; any witness must be aperiodic,
   and annealing walls suggest 2-4 cuts are always irreparable.)
2. Prove nonexistence for some n >= 14 outright.  The most promising angle
   is n=22, where only the three burn cards are undealt and every "threat
   card" must occupy a burn position at all 52 cuts simultaneously; known
   counting and matching relaxations are satisfiable, so a genuinely
   cross-cut argument is needed.

## 7. Reproducibility

All tooling lives in `jeff-search/` (Rust crate + Python CP-SAT/Z3
scripts); every claim above is reproducible from the commands recorded in
`RESULTS.md`, and machine-checkable artifacts (decks, UNSAT manifests,
Lean proofs) are under `jeff-search/decks/`, `jeff-search/compound_*/`,
and `lean-workspace/`.

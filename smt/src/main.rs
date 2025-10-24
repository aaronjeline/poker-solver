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

// Check if 5 cards form a straight
// Returns (is_straight, high_card_value)
// Handles special case: A-2-3-4-5 (wheel) where A acts as value 0
fn is_straight<'ctx>(ctx: &'ctx Context, cards: &[Int<'ctx>; 5]) -> (Bool<'ctx>, Int<'ctx>) {
    let values: Vec<Int> = cards.iter().map(|c| value(ctx, c)).collect();

    // We need to check if the 5 values form a consecutive sequence
    // This is complex in SMT. We'll check all possible straights.
    // Straights: 0-1-2-3-4 (wheel, A-2-3-4-5), 1-2-3-4-5, ..., 8-9-10-11-12 (10-J-Q-K-A)

    let mut straight_checks = Vec::new();

    // For each possible straight starting value
    for start in 0..=8 {
        // Check if we have exactly these 5 values: start, start+1, start+2, start+3, start+4
        let expected_vals: Vec<Int> = (0..5)
            .map(|i| Int::from_i64(ctx, start + i))
            .collect();

        // Check if values (in any order) match expected_vals
        let mut matches = Vec::new();
        for exp_val in &expected_vals {
            // At least one card must have this value
            let comparisons: Vec<Bool> = values.iter().map(|v| v._eq(exp_val)).collect();
            let comparisons_refs: Vec<&Bool> = comparisons.iter().collect();
            let any_match = Bool::or(ctx, &comparisons_refs);
            matches.push(any_match);
        }

        // Also ensure no duplicate values (implicitly satisfied if all 5 expected values are present)
        let is_this_straight = Bool::and(ctx, &matches.iter().collect::<Vec<_>>());

        straight_checks.push((is_this_straight, Int::from_i64(ctx, start + 4)));
    }

    // Special case: wheel (A-2-3-4-5) where A=12, but acts as low
    let wheel_values: Vec<Int> = vec![0, 1, 2, 3, 12].iter()
        .map(|&v| Int::from_i64(ctx, v))
        .collect();

    let mut wheel_matches = Vec::new();
    for exp_val in &wheel_values {
        let comparisons: Vec<Bool> = values.iter().map(|v| v._eq(exp_val)).collect();
        let comparisons_refs: Vec<&Bool> = comparisons.iter().collect();
        let any_match = Bool::or(ctx, &comparisons_refs);
        wheel_matches.push(any_match);
    }
    let is_wheel = Bool::and(ctx, &wheel_matches.iter().collect::<Vec<_>>());

    // High card for wheel is 3 (value=3, which is the card '5')
    straight_checks.push((is_wheel, Int::from_i64(ctx, 3)));

    // Combine all checks: is any straight true?
    let is_any_straight = Bool::or(ctx, &straight_checks.iter().map(|(c, _)| c).collect::<Vec<_>>());

    // Determine high card: use conditional selection
    let mut high_card = Int::from_i64(ctx, 0);
    for (i, (check, hc)) in straight_checks.iter().enumerate() {
        if i == 0 {
            high_card = check.ite(hc, &high_card);
        } else {
            high_card = check.ite(hc, &high_card);
        }
    }

    (is_any_straight, high_card)
}

// Count occurrences of each value (0-12) in the 5 cards
// Returns: (count array indices 0-12, sorted counts in descending order)
fn count_values<'ctx>(ctx: &'ctx Context, cards: &[Int<'ctx>; 5]) -> (Vec<Int<'ctx>>, Vec<Int<'ctx>>) {
    let values: Vec<Int> = cards.iter().map(|c| value(ctx, c)).collect();

    // For each value 0-12, count how many cards have it
    let mut counts = Vec::new();
    for val_id in 0..13 {
        let target = Int::from_i64(ctx, val_id);
        let mut count = Int::from_i64(ctx, 0);
        for v in &values {
            // count += (v == target) ? 1 : 0
            count = v._eq(&target).ite(&(count.clone() + Int::from_i64(ctx, 1)), &count);
        }
        counts.push(count);
    }

    // Extract unique counts and sort them (descending)
    // This is complex in SMT, so we'll do it manually for all 5 cards
    // Possible count patterns: [4,1], [3,2], [3,1,1], [2,2,1], [2,1,1,1], [1,1,1,1,1]

    // We would need to sort values by their counts (descending), then by value
    // but sorting in SMT is hard, so we skip this and use simplified tiebreakers
    let sorted_counts = values.iter()
        .map(|v| {
            let mut c = Int::from_i64(ctx, 0);
            for other_v in &values {
                c = v._eq(other_v).ite(&(c.clone() + Int::from_i64(ctx, 1)), &c);
            }
            c
        })
        .collect();

    (counts, sorted_counts)
}

// Determine hand rank and tiebreakers for 5 cards
// Returns: (rank, tiebreaker1, tiebreaker2, tiebreaker3, tiebreaker4, tiebreaker5)
// rank: 0=high card, 1=pair, 2=two pair, 3=three of a kind, 4=straight,
//       5=flush, 6=full house, 7=four of a kind, 8=straight flush
fn hand_rank<'ctx>(
    ctx: &'ctx Context,
    cards: &[Int<'ctx>; 5],
) -> (Int<'ctx>, Vec<Int<'ctx>>) {
    let values: Vec<Int> = cards.iter().map(|c| value(ctx, c)).collect();

    let is_flush_val = is_flush(ctx, cards);
    let (is_straight_val, _straight_high) = is_straight(ctx, cards);
    let (value_counts, _) = count_values(ctx, cards);

    // Count how many of each count we have
    let four_checks: Vec<Bool> = value_counts.iter().map(|c| c._eq(&Int::from_i64(ctx, 4))).collect();
    let four_refs: Vec<&Bool> = four_checks.iter().collect();
    let has_four = Bool::or(ctx, &four_refs);

    let three_checks: Vec<Bool> = value_counts.iter().map(|c| c._eq(&Int::from_i64(ctx, 3))).collect();
    let three_refs: Vec<&Bool> = three_checks.iter().collect();
    let has_three = Bool::or(ctx, &three_refs);

    // Count pairs
    let mut num_pairs = Int::from_i64(ctx, 0);
    for c in &value_counts {
        num_pairs = c._eq(&Int::from_i64(ctx, 2)).ite(
            &(num_pairs.clone() + Int::from_i64(ctx, 1)),
            &num_pairs
        );
    }
    let has_one_pair = num_pairs.clone()._eq(&Int::from_i64(ctx, 1));
    let has_two_pair = num_pairs._eq(&Int::from_i64(ctx, 2));

    let is_full_house = Bool::and(ctx, &[&has_three, &has_one_pair]);

    // Determine rank
    let mut rank = Int::from_i64(ctx, 0); // Default: high card

    // Straight flush (8)
    rank = Bool::and(ctx, &[&is_flush_val, &is_straight_val]).ite(&Int::from_i64(ctx, 8), &rank);

    // Four of a kind (7)
    rank = has_four.ite(&Int::from_i64(ctx, 7), &rank);

    // Full house (6)
    rank = is_full_house.ite(&Int::from_i64(ctx, 6), &rank);

    // Flush (5)
    rank = Bool::and(ctx, &[&is_flush_val, &is_straight_val.not()]).ite(&Int::from_i64(ctx, 5), &rank);

    // Straight (4)
    rank = Bool::and(ctx, &[&is_straight_val, &is_flush_val.not()]).ite(&Int::from_i64(ctx, 4), &rank);

    // Three of a kind (3)
    rank = Bool::and(ctx, &[&has_three, &has_one_pair.not()]).ite(&Int::from_i64(ctx, 3), &rank);

    // Two pair (2)
    rank = has_two_pair.ite(&Int::from_i64(ctx, 2), &rank);

    // One pair (1)
    rank = has_one_pair.ite(&Int::from_i64(ctx, 1), &rank);

    // Tiebreakers: for simplicity, we'll use sorted values in descending order
    // This isn't perfect (should prioritize by count pattern) but is a simplification
    // A proper implementation would sort them by (count, value) descending
    let tiebreakers = values;

    (rank, tiebreakers)
}

// Compare two hands: returns true if hand1 > hand2
fn hand_greater_than<'ctx>(
    ctx: &'ctx Context,
    rank1: &Int<'ctx>,
    tb1: &[Int<'ctx>],
    rank2: &Int<'ctx>,
    tb2: &[Int<'ctx>],
) -> Bool<'ctx> {
    // First compare ranks
    let rank_greater = rank1.gt(rank2);
    let rank_equal = rank1._eq(rank2);

    // If ranks equal, compare tiebreakers lexicographically
    let mut tb_greater = Bool::from_bool(ctx, false);
    let mut all_equal = Bool::from_bool(ctx, true);

    for i in 0..tb1.len().min(tb2.len()) {
        let this_greater = tb1[i].gt(&tb2[i]);
        let this_equal = tb1[i]._eq(&tb2[i]);

        // tb_greater = this_greater || (all_equal && this_greater)
        tb_greater = Bool::or(ctx, &[&tb_greater, &Bool::and(ctx, &[&all_equal, &this_greater])]);

        all_equal = Bool::and(ctx, &[&all_equal, &this_equal]);
    }

    Bool::or(ctx, &[&rank_greater, &Bool::and(ctx, &[&rank_equal, &tb_greater])])
}

// Get the best hand from 7 cards (2 hole + 5 community)
// Returns: (rank, tiebreakers)
fn best_hand_from_seven<'ctx>(
    ctx: &'ctx Context,
    hole_cards: &[Int<'ctx>; 2],
    community: &[Int<'ctx>; 5],
) -> (Int<'ctx>, Vec<Int<'ctx>>) {
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

    // Generate all 21 combinations of 5 cards from 7
    let combinations: Vec<[usize; 5]> = vec![
        [0,1,2,3,4], [0,1,2,3,5], [0,1,2,3,6], [0,1,2,4,5], [0,1,2,4,6],
        [0,1,2,5,6], [0,1,3,4,5], [0,1,3,4,6], [0,1,3,5,6], [0,1,4,5,6],
        [0,2,3,4,5], [0,2,3,4,6], [0,2,3,5,6], [0,2,4,5,6], [0,3,4,5,6],
        [1,2,3,4,5], [1,2,3,4,6], [1,2,3,5,6], [1,2,4,5,6], [1,3,4,5,6],
        [2,3,4,5,6],
    ];

    let mut best_rank = Int::from_i64(ctx, -1);
    let mut best_tiebreakers = vec![Int::from_i64(ctx, 0); 5];

    for combo in combinations {
        let hand = [
            all_cards[combo[0]].clone(),
            all_cards[combo[1]].clone(),
            all_cards[combo[2]].clone(),
            all_cards[combo[3]].clone(),
            all_cards[combo[4]].clone(),
        ];

        let (rank, tiebreakers) = hand_rank(ctx, &hand);

        // Update best if this is better
        let is_better = hand_greater_than(ctx, &rank, &tiebreakers, &best_rank, &best_tiebreakers);

        best_rank = is_better.ite(&rank, &best_rank);
        for i in 0..5 {
            best_tiebreakers[i] = is_better.ite(&tiebreakers[i], &best_tiebreakers[i]);
        }
    }

    (best_rank, best_tiebreakers)
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

        // Get best hand for each player
        let mut player_best_hands = Vec::new();
        for p in 0..n {
            let (rank, tiebreakers) = best_hand_from_seven(&ctx, &player_hands[p], &community);
            player_best_hands.push((rank, tiebreakers));
        }

        // Constraint: player 0 must win (beat all other players)
        for p in 1..n {
            let player0_better = hand_greater_than(
                &ctx,
                &player_best_hands[0].0,
                &player_best_hands[0].1,
                &player_best_hands[p].0,
                &player_best_hands[p].1,
            );
            solver.assert(&player0_better);
        }
    }

    println!();
    println!("All constraints generated!");
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

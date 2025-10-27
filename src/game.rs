use crate::deck::*;
use crate::hands::*;
use crate::precompute::*;

const MAX_WINS: usize = 52;

pub fn max_wins(real: bool) -> usize {
    if real { 52 - 10 } else { 52 }
}

pub fn num_wins(num_players: usize, deck: &Deck, table: &ScoreTable, real: bool) -> usize {
    if !real {
        num_wins_total(num_players, deck, table)
    } else {
        num_realistic_wins(num_players, deck, table)
    }
}

pub fn num_wins_total(num_players: usize, deck: &Deck, table: &ScoreTable) -> usize {
    (0..52)
        .filter(|cut_pos| dealer_wins_game(num_players, deck.clone().cut(*cut_pos), table))
        .count()
}

pub fn num_realistic_wins(num_players: usize, deck: &Deck, table: &ScoreTable) -> usize {
    (5..47)
        .filter(|cut_pos| dealer_wins_game(num_players, deck.clone().cut(*cut_pos), table))
        .count()
}

pub fn dealer_wins_game(num_players: usize, deck: Deck, table: &ScoreTable) -> bool {
    deal_a_round(num_players, deck).dealer_wins(table)
}

/// Hybrid scoring function that combines win count with margin of victory
/// Returns: (num_wins * WIN_WEIGHT) + total_margin
/// This provides a smooth gradient for optimization while prioritizing wins
pub fn hybrid_score(num_players: usize, deck: &Deck, table: &ScoreTable, real: bool) -> f64 {
    const WIN_WEIGHT: f64 = 100_000.0; // One win is worth 100k points

    let positions: Vec<usize> = if real {
        (5..47).collect()
    } else {
        (0..52).collect()
    };

    let mut num_wins = 0;
    let mut total_margin = 0.0;

    for cut_pos in positions {
        let cut_deck = deck.clone().cut(cut_pos);
        let game = deal_a_round(num_players, cut_deck);

        // Get player 0's score
        let p0_score = game.players_score(0, table);

        // Get best opponent's score
        let best_opponent_score = (1..num_players)
            .map(|idx| game.players_score(idx, table))
            .max()
            .unwrap();

        // Calculate margin (positive if player 0 wins)
        let margin = p0_score.to_score() - best_opponent_score.to_score();

        if margin > 0 {
            num_wins += 1;
        }

        total_margin += margin as f64;
    }

    // Hybrid score: heavily weight wins, but use margins as tiebreaker/gradient
    (num_wins as f64) * WIN_WEIGHT + total_margin
}

/// Get just the margin component for a single cut position
pub fn position_margin(num_players: usize, deck: &Deck, cut_pos: usize, table: &ScoreTable) -> i32 {
    let cut_deck = deck.clone().cut(cut_pos);
    let game = deal_a_round(num_players, cut_deck);

    let p0_score = game.players_score(0, table);
    let best_opponent_score = (1..num_players)
        .map(|idx| game.players_score(idx, table))
        .max()
        .unwrap();

    p0_score.to_score() - best_opponent_score.to_score()
}

struct Game {
    players: Vec<Player>,
    common: Common,
}

impl Game {
    pub fn dealer_wins(&self, table: &ScoreTable) -> bool {
        self.winning_player(table) == 0
    }

    pub fn winning_player(&self, table: &ScoreTable) -> usize {
        let winner = (0..self.players.len())
            .max_by_key(|idx| self.players_score(*idx, table))
            .unwrap();
        winner
    }

    pub fn players_score(&self, idx: usize, table: &ScoreTable) -> TableEntry {
        let p = &self.players[idx];

        // With 7-card precomputation, we directly look up the score
        // for the player's 2 hole cards + 5 community cards
        let mut cards = [
            p.0[0],
            p.0[1],
            self.common.0[0],
            self.common.0[1],
            self.common.0[2],
            self.common.0[3],
            self.common.0[4],
        ];
        cards.sort();
        let hand = Hand(cards);
        table.score(&hand)
    }
}

pub fn deal_a_round(num_players: usize, mut deck: Deck) -> Game {
    let mut players = vec![Player::default(); num_players];
    let mut common = Common::default();
    for hand_idx in 0..2 {
        for p in 0..num_players {
            let card = deck.draw();
            players[p].0[hand_idx] = card;
        }
    }
    let _burn = deck.draw();
    common.0[0] = deck.draw();
    common.0[1] = deck.draw();
    common.0[2] = deck.draw();
    let _burn = deck.draw();
    common.0[3] = deck.draw();
    let _burn = deck.draw();
    common.0[4] = deck.draw();

    Game { players, common }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::*;
    use crate::precompute::load_table;
    use std::fs::File;

    #[test]
    #[ignore = "Requires regenerating hands file with 7-card precomputation"]
    fn test_player1_wins_with_straight() {
        // Community: 4♣, 3♦, 7♠, 5♣, J♠
        // Player 1: K♥, 6♦
        // Player 0: 8♥, 7♦
        //
        // Expected outcome:
        // Player 0: Best hand is pair of 7s (7♦ from hand + 7♠ from community)
        // Player 1: Best hand is straight 3-4-5-6-7 (6♦ from hand + 3♦,4♣,5♣,7♠ from community)
        //
        // Player 1 should win with a straight (rank 5) vs Player 0's pair (rank 2)

        // Create the cards
        let card_4c = Card::new(Value::new(4), Suit::Clubs);    // 4♣
        let card_3d = Card::new(Value::new(3), Suit::Diamonds); // 3♦
        let card_7s = Card::new(Value::new(7), Suit::Spades);   // 7♠
        let card_5c = Card::new(Value::new(5), Suit::Clubs);    // 5♣
        let card_js = Card::new(Value::new(11), Suit::Spades);  // J♠

        let card_kh = Card::new(Value::new(13), Suit::Hearts);  // K♥
        let card_6d = Card::new(Value::new(6), Suit::Diamonds); // 6♦

        let card_8h = Card::new(Value::new(8), Suit::Hearts);   // 8♥
        let card_7d = Card::new(Value::new(7), Suit::Diamonds); // 7♦

        // Create players
        let player0 = Player([card_8h, card_7d]);
        let player1 = Player([card_kh, card_6d]);

        // Create common cards
        let common = Common([card_4c, card_3d, card_7s, card_5c, card_js]);

        // Create game
        let game = Game {
            players: vec![player0, player1],
            common,
        };

        // Load the precomputed score table
        let file = File::open("hands").expect("Failed to open hands file");
        let table = load_table(file).expect("Failed to load hands file");

        // Get scores for debugging
        let p0_score = game.players_score(0, &table);
        let p1_score = game.players_score(1, &table);

        println!("Player 0 (8♥, 7♦) score: rank={}, hi={}", p0_score.rank, p0_score.hi);
        println!("Player 1 (K♥, 6♦) score: rank={}, hi={}", p1_score.rank, p1_score.hi);

        // Verify that player 1 wins
        assert_eq!(p0_score.rank, 2, "Player 0 should have a pair");
        assert_eq!(p1_score.rank, 5, "Player 1 should have a straight");
        assert!(!game.dealer_wins(&table), "Player 0 (dealer) should NOT win this hand");
        assert_eq!(game.winning_player(&table), 1, "Player 1 should be the winning player");
    }

    #[test]
    #[ignore = "Requires regenerating hands file with 7-card precomputation"]
    fn test_player1_wins_with_ace_high() {
        // Community: 7♥, 5♣, 10♣, 8♣, 8♥
        // Player 1: A♣, 3♦
        // Player 0: Q♦, 4♠
        //
        // Expected outcome:
        // Player 1: Best hand is pair of 8s with Ace high: 8♣, 8♥, A♣, 10♣, 7♥
        // Player 0: Best hand is pair of 8s with Queen high: 8♣, 8♥, Q♦, 10♣, 7♥
        //
        // Both have pair of 8s (rank 2), but Player 1 has Ace high (14) vs Player 0's Queen high (12)
        // So Player 1 should win!

        // Create the cards
        let card_7h = Card::new(Value::new(7), Suit::Hearts);   // 7♥
        let card_5c = Card::new(Value::new(5), Suit::Clubs);    // 5♣
        let card_10c = Card::new(Value::new(10), Suit::Clubs);  // 10♣
        let card_8c = Card::new(Value::new(8), Suit::Clubs);    // 8♣
        let card_8h = Card::new(Value::new(8), Suit::Hearts);   // 8♥

        let card_ac = Card::new(Value::new(1), Suit::Clubs);    // A♣ (Ace = 1)
        let card_3d = Card::new(Value::new(3), Suit::Diamonds); // 3♦

        let card_qd = Card::new(Value::new(12), Suit::Diamonds); // Q♦
        let card_4s = Card::new(Value::new(4), Suit::Spades);    // 4♠

        // Create players
        let player0 = Player([card_qd, card_4s]);
        let player1 = Player([card_ac, card_3d]);

        // Create common cards
        let common = Common([card_7h, card_5c, card_10c, card_8c, card_8h]);

        // Create game
        let game = Game {
            players: vec![player0, player1],
            common,
        };

        // Load the precomputed score table
        let file = File::open("hands").expect("Failed to open hands file");
        let table = load_table(file).expect("Failed to load hands file");

        // Get scores for debugging
        let p0_score = game.players_score(0, &table);
        let p1_score = game.players_score(1, &table);

        println!("Player 0 (Q♦, 4♠) score: rank={}, hi={}", p0_score.rank, p0_score.hi);
        println!("Player 1 (A♣, 3♦) score: rank={}, hi={}", p1_score.rank, p1_score.hi);

        // Let me manually check the best hands using the full 7-card hands
        use crate::hands::Hand;

        // Player 0 full 7-card hand: Q♦, 4♠ (hole) + 7♥, 5♣, 10♣, 8♣, 8♥ (community)
        let mut p0_full = [card_qd, card_4s, card_7h, card_5c, card_10c, card_8c, card_8h];
        p0_full.sort();
        let p0_hand = Hand(p0_full);
        let p0_manual = p0_hand.score();
        println!("Player 0 manual 7-card: rank={}, hi={}", p0_manual.rank, p0_manual.hi);

        // Player 1 full 7-card hand: A♣, 3♦ (hole) + 7♥, 5♣, 10♣, 8♣, 8♥ (community)
        let mut p1_full = [card_ac, card_3d, card_7h, card_5c, card_10c, card_8c, card_8h];
        p1_full.sort();
        println!("Player 1 sorted cards: {:?}", p1_full.iter().map(|c| c.into_inner()).collect::<Vec<_>>());
        let p1_hand = Hand(p1_full);
        let p1_manual = p1_hand.score();
        println!("Player 1 manual 7-card: rank={}, hi={}", p1_manual.rank, p1_manual.hi);

        // Verify that player 1 wins (both have pair of 8s, but Player 1 has Ace high)
        assert_eq!(p0_score.rank, 2, "Player 0 should have a pair");
        assert_eq!(p1_score.rank, 2, "Player 1 should have a pair");
        assert_eq!(p1_score.hi, 14, "Player 1's high card should be Ace (14)");
        assert_eq!(p0_score.hi, 12, "Player 0's high card should be Queen (12)");
        assert!(!game.dealer_wins(&table), "Player 0 (dealer) should NOT win this hand");
        assert_eq!(game.winning_player(&table), 1, "Player 1 should be the winning player");
    }

    #[test]
    fn test_player0_wins_specific_hand_direct_scoring() {
        // Same test but using Hand::score() directly instead of precomputed table
        // Community: 4♣, 3♦, 7♠, 5♣, J♠
        // Player 1: K♥, 6♦
        // Player 0: 8♥, 7♦

        use crate::hands::Hand;

        // Create the cards
        let card_4c = Card::new(Value::new(4), Suit::Clubs);    // 4♣
        let card_3d = Card::new(Value::new(3), Suit::Diamonds); // 3♦
        let card_7s = Card::new(Value::new(7), Suit::Spades);   // 7♠
        let card_5c = Card::new(Value::new(5), Suit::Clubs);    // 5♣
        let card_js = Card::new(Value::new(11), Suit::Spades);  // J♠

        let card_kh = Card::new(Value::new(13), Suit::Hearts);  // K♥
        let card_6d = Card::new(Value::new(6), Suit::Diamonds); // 6♦

        let card_8h = Card::new(Value::new(8), Suit::Hearts);   // 8♥
        let card_7d = Card::new(Value::new(7), Suit::Diamonds); // 7♦

        // Check Player 1's full 7-card hand: K♥, 6♦ (hole) + 4♣, 3♦, 7♠, 5♣, J♠ (community)
        // This should find the 3-4-5-6-7 straight
        let mut p1_full = [card_kh, card_6d, card_4c, card_3d, card_7s, card_5c, card_js];
        p1_full.sort(); // Hands must be sorted
        let p1_hand = Hand(p1_full);
        let p1_score = p1_hand.score();
        println!("Player 1 7-card (has 3-4-5-6-7 straight): rank={}, hi={}", p1_score.rank, p1_score.hi);

        // Check Player 0's full 7-card hand: 8♥, 7♦ (hole) + 4♣, 3♦, 7♠, 5♣, J♠ (community)
        // This should find the pair of 7s
        let mut p0_full = [card_8h, card_7d, card_4c, card_3d, card_7s, card_5c, card_js];
        p0_full.sort(); // Hands must be sorted
        let p0_hand = Hand(p0_full);
        let p0_score = p0_hand.score();
        println!("Player 0 7-card (has pair of 7s): rank={}, hi={}", p0_score.rank, p0_score.hi);

        // Straight (rank 5) should beat pair (rank 2)
        assert_eq!(p1_score.rank, 5, "Player 1 should have a straight");
        assert_eq!(p0_score.rank, 2, "Player 0 should have a pair");
        assert!(p1_score.rank > p0_score.rank, "Player 1's straight should beat Player 0's pair");
    }
}

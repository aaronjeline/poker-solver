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
        // Generate all C(5,3) = 10 combinations of 3 cards from common
        let mut hands: Vec<Hand> = Vec::with_capacity(10);

        // Iterate through all combinations of 3 indices from [0,1,2,3,4]
        for i in 0..5 {
            for j in (i + 1)..5 {
                for k in (j + 1)..5 {
                    // Create a hand with player's 2 cards + 3 common cards
                    let mut cards = [
                        p.0[0],
                        p.0[1],
                        self.common.0[i],
                        self.common.0[j],
                        self.common.0[k],
                    ];
                    // Sort the cards to ensure proper Hand format
                    cards.sort();
                    hands.push(Hand(cards));
                }
            }
        }

        // Find the best hand score
        hands.iter().map(|hand| table.score(hand)).max().unwrap()
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

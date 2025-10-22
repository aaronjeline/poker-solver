use crate::deck::*;
use crate::hands::*;
use crate::precompute::*;

pub const MAX_WINS: usize = 52;

pub fn num_wins(num_players: usize, deck: &Deck, table: &ScoreTable) -> usize {
    (0..52)
        .filter(|cut_pos| dealer_wins_game(num_players, deck.clone().cut(*cut_pos), table))
        .count()
}

fn dealer_wins_game(num_players: usize, deck: Deck, table: &ScoreTable) -> bool {
    deal_a_round(num_players, deck).dealer_wins(table)
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

    fn players_score(&self, idx: usize, table: &ScoreTable) -> TableEntry {
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
    for i in 0..5 {
        common.0[i] = deck.draw();
    }
    Game { players, common }
}

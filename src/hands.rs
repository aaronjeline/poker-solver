use crate::cards::*;
use crate::precompute::Entry;

pub const HAND_SIZE: usize = 7;
pub const ALL_HANDS: usize = 133_784_560;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Hand(pub [Card; 7]);

/// Calculate binomial coefficient C(n, k)
fn binomial(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    if k == 0 {
        return 1;
    }

    let mut result = 1;
    for i in 0..k {
        result = result * (n - i) / (i + 1);
    }
    result
}

impl Hand {
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: Cards are a newtype of u8
        unsafe { std::mem::transmute(self.0.as_slice()) }
    }

    /// Compute the rank of this hand (higher is better)
    /// Also return the rank of the high card
    /// This _will not_ be called in a hot loop, and will be used to precompute a lookup table
    ///
    /// For 7-card hands, we find the best 5-card poker hand within the 7 cards
    pub fn score(self) -> Entry {
        // For 7 cards, we need to check all C(7,5) = 21 possible 5-card combinations
        // and find the best one
        let mut best_rank = 0u8;
        let mut best_hi = 0u8;

        // Iterate through all 5-card combinations from the 7 cards
        for i in 0..7 {
            for j in (i + 1)..7 {
                for k in (j + 1)..7 {
                    for l in (k + 1)..7 {
                        for m in (l + 1)..7 {
                            let five_card_hand = [
                                self.0[i],
                                self.0[j],
                                self.0[k],
                                self.0[l],
                                self.0[m],
                            ];

                            let (rank, hi) = score_five_cards(five_card_hand);

                            // Update best if this hand is better
                            if rank > best_rank || (rank == best_rank && hi > best_hi) {
                                best_rank = rank;
                                best_hi = hi;
                            }
                        }
                    }
                }
            }
        }

        Entry {
            hand: self,
            rank: best_rank,
            hi: best_hi,
        }
    }
}

/// Score a 5-card poker hand
/// Returns (rank, high_card)
fn score_five_cards(cards: [Card; 5]) -> (u8, u8) {
    // Extract values and suits from all cards
    let mut values = [0u8; 5];
    let mut suits = [0u8; 5];

    for i in 0..5 {
        let (value, suit) = cards[i].into_inner();
        values[i] = value.0;
        suits[i] = suit.into();
    }

    // Sort values for easier analysis
    values.sort_unstable();

    // Find high card (last after sorting)
    // Special case: Ace (value 1) should be treated as 14 (highest) for high card
    let high_card = if values[0] == 1 {
        // If we have an Ace, it's the high card (treat as 14)
        // unless we have a 5-high straight (A-2-3-4-5)
        if values == [1, 2, 3, 4, 5] {
            // 5-high straight, high card is 5
            5
        } else {
            // Ace is high
            14
        }
    } else {
        // No Ace, high card is last element
        values[4]
    };

    // Check for flush (all same suit)
    let is_flush = suits.iter().all(|&s| s == suits[0]);

    // Check for straight
    let is_straight = if values == [1, 10, 11, 12, 13] {
        // Ace-high straight (royal)
        true
    } else {
        // Regular straight: consecutive values
        values[1] == values[0] + 1
            && values[2] == values[1] + 1
            && values[3] == values[2] + 1
            && values[4] == values[3] + 1
    };

    // Count frequency of each unique value
    // First, collect unique values and their counts
    let mut unique_values = [0u8; 5];
    let mut unique_counts = [0u8; 5];
    let mut num_unique = 0usize;

    for i in 0..5 {
        // Check if this value is already counted
        let mut found = false;
        for j in 0..num_unique {
            if unique_values[j] == values[i] {
                found = true;
                break;
            }
        }

        if !found {
            unique_values[num_unique] = values[i];
            // Count how many times this value appears
            let mut count = 0;
            for j in 0..5 {
                if values[j] == values[i] {
                    count += 1;
                }
            }
            unique_counts[num_unique] = count;
            num_unique += 1;
        }
    }

    // Sort counts in descending order
    let mut sorted_counts = unique_counts;
    sorted_counts[0..num_unique].sort_unstable_by(|a, b| b.cmp(a));

    // Determine hand rank
    let rank = match (
        sorted_counts[0],
        sorted_counts.get(1).copied().unwrap_or(0),
        is_flush,
        is_straight,
    ) {
        (_, _, true, true) => 9,  // Straight Flush
        (4, _, _, _) => 8,        // Four of a Kind
        (3, 2, _, _) => 7,        // Full House
        (_, _, true, false) => 6, // Flush
        (_, _, false, true) => 5, // Straight
        (3, _, _, _) => 4,        // Three of a Kind
        (2, 2, _, _) => 3,        // Two Pair
        (2, _, _, _) => 2,        // One Pair
        _ => 1,                   // High Card
    };

    (rank, high_card)
}

pub fn all_hands() -> impl Iterator<Item = Hand> {
    Hands::new()
}

struct Hands {
    // Current state: 7 card indices in increasing order
    // None means iteration is complete
    state: Option<[u8; 7]>,
}

impl Hands {
    pub fn new() -> Self {
        // Start with the first combination: [0, 1, 2, 3, 4, 5, 6]
        Self {
            state: Some([0, 1, 2, 3, 4, 5, 6]),
        }
    }
}

// An iterator over all possible hands
// A hand cannot contain the same card twice
impl Iterator for Hands {
    type Item = Hand;
    fn next(&mut self) -> Option<Self::Item> {
        // Get current state, return None if exhausted
        let current = self.state?;

        // Create the hand from current state
        let hand = Hand([
            Card(current[0]),
            Card(current[1]),
            Card(current[2]),
            Card(current[3]),
            Card(current[4]),
            Card(current[5]),
            Card(current[6]),
        ]);

        // Increment to next combination
        // Find the rightmost index that can be incremented
        const MAX_CARD: u8 = 51;
        let mut pos = None;

        for i in (0..7).rev() {
            // Check if this position can be incremented
            // Position i can be incremented if current[i] < MAX_CARD - (6 - i)
            if current[i] < MAX_CARD - (6 - i) as u8 {
                pos = Some(i);
                break;
            }
        }

        match pos {
            Some(i) => {
                // Increment position i and reset all positions to the right
                let mut next_state = current;
                next_state[i] += 1;
                for j in (i + 1)..7 {
                    next_state[j] = next_state[j - 1] + 1;
                }
                self.state = Some(next_state);
            }
            None => {
                // No position can be incremented, we're done
                self.state = None;
            }
        }

        Some(hand)
    }
}

/// Represents Hole Cards
#[derive(Debug, Clone, Copy, Default)]
pub struct Player(pub [Card; 2]);

#[derive(Debug, Clone, Default)]
pub struct Common(pub [Card; 5]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hands_iterator_count() {
        let hands = Hands::new();
        let count = hands.count();

        // C(52, 7) = 52! / (7! * 45!) = 133,784,560
        assert_eq!(count, ALL_HANDS);
    }

    #[test]
    fn test_hands_no_duplicates() {
        let hands = Hands::new();

        // Check first 1000 hands have no duplicate cards
        for hand in hands.take(1000) {
            let cards = &hand.0;
            for i in 0..7 {
                for j in (i + 1)..7 {
                    assert_ne!(cards[i].0, cards[j].0, "Hand contains duplicate cards");
                }
            }
        }
    }

    #[test]
    fn test_hands_in_order() {
        let hands = Hands::new();

        // Check first 1000 hands have cards in increasing order
        for hand in hands.take(1000) {
            let cards = &hand.0;
            for i in 0..6 {
                assert!(cards[i].0 < cards[i + 1].0, "Cards not in increasing order");
            }
        }
    }

    #[test]
    fn test_score_high_card() {
        // High card: 2♣ 4♦ 6♠ 9♥ K♣ + two extra cards 3♥ 8♦
        // (avoiding consecutive cards to prevent straights)
        let hand = Hand([
            Card::new(Value::new(2), Suit::Clubs),
            Card::new(Value::new(3), Suit::Hearts),
            Card::new(Value::new(4), Suit::Diamonds),
            Card::new(Value::new(6), Suit::Spades),
            Card::new(Value::new(8), Suit::Diamonds),
            Card::new(Value::new(9), Suit::Hearts),
            Card::new(Value::new(13), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 1); // High card
        assert_eq!(entry.hi, 13); // King
    }

    #[test]
    fn test_score_one_pair() {
        // One pair: 3♣ 3♦ 5♠ 7♥ 9♣ + two extra cards 2♥ 4♦
        let hand = Hand([
            Card::new(Value::new(2), Suit::Hearts),
            Card::new(Value::new(3), Suit::Clubs),
            Card::new(Value::new(3), Suit::Diamonds),
            Card::new(Value::new(4), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Spades),
            Card::new(Value::new(7), Suit::Hearts),
            Card::new(Value::new(9), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 2); // One pair
        assert_eq!(entry.hi, 9);
    }

    #[test]
    fn test_score_two_pair() {
        // Two pair: 3♣ 3♦ 7♠ 7♥ 9♣ + two extra cards 2♥ 4♦
        let hand = Hand([
            Card::new(Value::new(2), Suit::Hearts),
            Card::new(Value::new(3), Suit::Clubs),
            Card::new(Value::new(3), Suit::Diamonds),
            Card::new(Value::new(4), Suit::Diamonds),
            Card::new(Value::new(7), Suit::Spades),
            Card::new(Value::new(7), Suit::Hearts),
            Card::new(Value::new(9), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 3); // Two pair
        assert_eq!(entry.hi, 9);
    }

    #[test]
    fn test_score_three_of_a_kind() {
        // Three of a kind: 5♣ 5♦ 5♠ 7♥ 9♣ + two extra cards 2♥ 3♦
        let hand = Hand([
            Card::new(Value::new(2), Suit::Hearts),
            Card::new(Value::new(3), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Clubs),
            Card::new(Value::new(5), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Spades),
            Card::new(Value::new(7), Suit::Hearts),
            Card::new(Value::new(9), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 4); // Three of a kind
        assert_eq!(entry.hi, 9);
    }

    #[test]
    fn test_score_straight() {
        // Straight: 5♣ 6♦ 7♠ 8♥ 9♣ + two extra cards 2♥ 3♦
        let hand = Hand([
            Card::new(Value::new(2), Suit::Hearts),
            Card::new(Value::new(3), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Clubs),
            Card::new(Value::new(6), Suit::Diamonds),
            Card::new(Value::new(7), Suit::Spades),
            Card::new(Value::new(8), Suit::Hearts),
            Card::new(Value::new(9), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 5); // Straight
        assert_eq!(entry.hi, 9);
    }

    #[test]
    fn test_score_flush() {
        // Flush: 2♣ 5♣ 7♣ 9♣ K♣ + two extra cards 3♥ 4♦
        let hand = Hand([
            Card::new(Value::new(2), Suit::Clubs),
            Card::new(Value::new(3), Suit::Hearts),
            Card::new(Value::new(4), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Clubs),
            Card::new(Value::new(7), Suit::Clubs),
            Card::new(Value::new(9), Suit::Clubs),
            Card::new(Value::new(13), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 6); // Flush
        assert_eq!(entry.hi, 13);
    }

    #[test]
    fn test_score_full_house() {
        // Full house: 5♣ 5♦ 5♠ 7♥ 7♣ + two extra cards 2♥ 3♦
        let hand = Hand([
            Card::new(Value::new(2), Suit::Hearts),
            Card::new(Value::new(3), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Clubs),
            Card::new(Value::new(5), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Spades),
            Card::new(Value::new(7), Suit::Hearts),
            Card::new(Value::new(7), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 7); // Full house
        assert_eq!(entry.hi, 7);
    }

    #[test]
    fn test_score_four_of_a_kind() {
        // Four of a kind: 5♣ 5♦ 5♠ 5♥ 9♣ + two extra cards 2♥ 3♦
        let hand = Hand([
            Card::new(Value::new(2), Suit::Hearts),
            Card::new(Value::new(3), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Clubs),
            Card::new(Value::new(5), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Spades),
            Card::new(Value::new(5), Suit::Hearts),
            Card::new(Value::new(9), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 8); // Four of a kind
        assert_eq!(entry.hi, 9);
    }

    #[test]
    fn test_score_straight_flush() {
        // Straight flush: 5♣ 6♣ 7♣ 8♣ 9♣ + two extra cards 2♥ 3♦
        let hand = Hand([
            Card::new(Value::new(2), Suit::Hearts),
            Card::new(Value::new(3), Suit::Diamonds),
            Card::new(Value::new(5), Suit::Clubs),
            Card::new(Value::new(6), Suit::Clubs),
            Card::new(Value::new(7), Suit::Clubs),
            Card::new(Value::new(8), Suit::Clubs),
            Card::new(Value::new(9), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 9); // Straight flush
        assert_eq!(entry.hi, 9);
    }

    #[test]
    fn test_score_royal_flush() {
        // Royal flush: 10♣ J♣ Q♣ K♣ A♣ + two extra cards 2♥ 3♦
        let hand = Hand([
            Card::new(Value::new(1), Suit::Clubs),
            Card::new(Value::new(2), Suit::Hearts),
            Card::new(Value::new(3), Suit::Diamonds),
            Card::new(Value::new(10), Suit::Clubs),
            Card::new(Value::new(11), Suit::Clubs),
            Card::new(Value::new(12), Suit::Clubs),
            Card::new(Value::new(13), Suit::Clubs),
        ]);
        let entry = hand.score();
        assert_eq!(entry.rank, 9); // Straight flush (royal flush is the highest straight flush)
        assert_eq!(entry.hi, 14); // Ace high (14)
    }
}

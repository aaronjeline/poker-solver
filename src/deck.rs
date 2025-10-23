use oorandom::Rand32;

use crate::cards::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deck(pub Vec<Card>);

impl Deck {
    pub fn apply_mutations(mut self, mutations: impl Iterator<Item = Mutation>) -> Self {
        for mutation in mutations {
            self = self.apply_mutation(mutation);
        }
        self
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.0.swap(a, b);
    }

    pub fn apply_mutation(mut self, mutation: Mutation) -> Self {
        self.0.swap(mutation.0.0, mutation.0.1);
        self
    }

    pub fn cut(mut self, pos: usize) -> Self {
        let mut taken = self.0.drain(0..pos).collect::<Vec<_>>();
        self.0.append(&mut taken);
        self
    }

    pub fn draw(&mut self) -> Card {
        self.0.pop().unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn new_deck_order() -> Deck {
        let mut v = vec![];
        v.reserve(52);

        for value in 1..=13 {
            for suit in 0..4 {
                let card = Card::new(Value::new(value), suit.into());
                v.push(card);
            }
        }
        assert!(v.len() == 52);

        Self(v)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn shuffle(mut self, rand: &mut Rand32) -> Deck {
        let n = self.0.len() as u32;

        for i in 0..(n - 1) {
            let j = rand.rand_range(i..n) as usize;
            self.0.swap(i as usize, j as usize);
        }
        self
    }

    /// Two-point crossover: takes a segment from parent1 and fills remaining positions with parent2's cards
    pub fn crossover(parent1: &Deck, parent2: &Deck, rng: &mut Rand32) -> Deck {
        let deck_size = parent1.0.len();

        // Choose two random crossover points
        let point1 = rng.rand_range(0..deck_size as u32) as usize;
        let point2 = rng.rand_range(0..deck_size as u32) as usize;
        let (start, end) = if point1 < point2 {
            (point1, point2)
        } else {
            (point2, point1)
        };

        // Start with parent1's segment between the crossover points
        let mut child = vec![None; deck_size];
        for i in start..end {
            child[i] = Some(parent1.0[i]);
        }

        // Fill remaining positions with parent2's cards in order, skipping duplicates
        let mut parent2_idx = 0;
        for i in 0..deck_size {
            if child[i].is_none() {
                // Find next card from parent2 that's not already in child
                while let Some(_) = child.iter().find(|&&c| c == Some(parent2.0[parent2_idx])) {
                    parent2_idx += 1;
                    if parent2_idx >= deck_size {
                        break;
                    }
                }
                if parent2_idx < deck_size {
                    child[i] = Some(parent2.0[parent2_idx]);
                    parent2_idx += 1;
                }
            }
        }

        Deck(child.into_iter().map(|c| c.unwrap()).collect())
    }

    /// Uniform crossover: each position randomly chosen from either parent
    /// This maintains valid decks by using order-based crossover
    pub fn uniform_crossover(parent1: &Deck, parent2: &Deck, rng: &mut Rand32) -> Deck {
        let deck_size = parent1.0.len();
        let mut child = vec![None; deck_size];

        // Randomly select positions to inherit from parent1
        for i in 0..deck_size {
            if rng.rand_range(0..2) == 0 {
                child[i] = Some(parent1.0[i]);
            }
        }

        // Fill remaining positions with parent2's cards in order, skipping duplicates
        let mut parent2_idx = 0;
        for i in 0..deck_size {
            if child[i].is_none() {
                // Find next card from parent2 that's not already in child
                while let Some(_) = child.iter().find(|&&c| c == Some(parent2.0[parent2_idx])) {
                    parent2_idx += 1;
                    if parent2_idx >= deck_size {
                        break;
                    }
                }
                if parent2_idx < deck_size {
                    child[i] = Some(parent2.0[parent2_idx]);
                    parent2_idx += 1;
                }
            }
        }

        Deck(child.into_iter().map(|c| c.unwrap()).collect())
    }
}

impl std::fmt::Display for Deck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, card) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", card)?;
        }
        write!(f, "]")
    }
}

pub fn generate_mutations(rng: &mut Rand32) -> impl Iterator<Item = Mutation> {
    let num_mutations = rng.rand_range(1..4);
    let mut muts = vec![];
    for _ in 0..num_mutations {
        muts.push(Mutation::generate(rng));
    }
    muts.into_iter()
}

pub fn generate_adaptive_mutations(rng: &mut Rand32, mutation_rate: f32) -> Vec<AdvancedMutation> {
    // Number of mutations scales with mutation_rate
    let num_mutations = if mutation_rate > 0.2 {
        rng.rand_range(2..5) as usize
    } else {
        rng.rand_range(1..3) as usize
    };

    let mut muts = vec![];
    for _ in 0..num_mutations {
        muts.push(AdvancedMutation::generate(rng, mutation_rate));
    }
    muts
}

#[derive(Debug, Clone, Copy)]
pub struct Mutation((usize, usize));

impl Mutation {
    pub fn generate(rng: &mut Rand32) -> Self {
        let a = rng.rand_range(0..52);
        let b = rng.rand_range(0..52);
        let end = a.max(b);
        let start = a.min(b);

        Self((start as usize, end as usize))
    }
}

#[derive(Debug, Clone)]
pub enum AdvancedMutation {
    Swap(usize, usize),
    BlockSwap(usize, usize, usize), // start1, start2, length
    Reversal(usize, usize),         // start, end
    Rotation(usize),                // cut position
    Scramble(usize, usize),         // start, end - shuffle this segment
}

impl AdvancedMutation {
    pub fn generate(rng: &mut Rand32, mutation_rate: f32) -> Self {
        // Higher mutation rate = more aggressive mutations
        let mutation_type = if mutation_rate > 0.2 {
            // When stuck, use more aggressive mutations
            rng.rand_range(0..5)
        } else {
            // When progressing, favor simpler mutations (swap, reversal)
            match rng.rand_range(0..10) {
                0..=5 => 0, // Swap
                6..=8 => 2, // Reversal
                _ => 1,     // BlockSwap
            }
        };

        match mutation_type {
            0 => {
                // Simple swap
                let a = rng.rand_range(0..52) as usize;
                let b = rng.rand_range(0..52) as usize;
                AdvancedMutation::Swap(a, b)
            }
            1 => {
                // Block swap - swap two segments of cards
                let len = rng.rand_range(2..8) as usize;
                let start1 = rng.rand_range(0..(52 - len) as u32) as usize;
                let start2 = rng.rand_range(0..(52 - len) as u32) as usize;
                AdvancedMutation::BlockSwap(start1, start2, len)
            }
            2 => {
                // Reversal - reverse a segment
                let a = rng.rand_range(0..52) as usize;
                let b = rng.rand_range(0..52) as usize;
                let start = a.min(b);
                let end = a.max(b);
                AdvancedMutation::Reversal(start, end)
            }
            3 => {
                // Rotation - cut the deck
                let pos = rng.rand_range(1..52) as usize;
                AdvancedMutation::Rotation(pos)
            }
            _ => {
                // Scramble - shuffle a segment
                let a = rng.rand_range(0..52) as usize;
                let b = rng.rand_range(0..52) as usize;
                let start = a.min(b);
                let end = a.max(b).min(start + 10); // Limit scramble size
                AdvancedMutation::Scramble(start, end)
            }
        }
    }

    pub fn apply(self, mut deck: Deck, rng: &mut Rand32) -> Deck {
        match self {
            AdvancedMutation::Swap(i, j) => {
                deck.0.swap(i, j);
                deck
            }
            AdvancedMutation::BlockSwap(start1, start2, len) => {
                if start1 + len > 52 || start2 + len > 52 || start1 == start2 {
                    return deck; // Invalid, return unchanged
                }
                // Swap blocks by using a temporary buffer
                for i in 0..len {
                    deck.0.swap(start1 + i, start2 + i);
                }
                deck
            }
            AdvancedMutation::Reversal(start, end) => {
                if start < end && end <= 52 {
                    deck.0[start..end].reverse();
                }
                deck
            }
            AdvancedMutation::Rotation(pos) => deck.cut(pos),
            AdvancedMutation::Scramble(start, end) => {
                if start < end && end <= 52 {
                    // Fisher-Yates shuffle on the segment
                    for i in start..end {
                        let j = rng.rand_range(i as u32..end as u32) as usize;
                        deck.0.swap(i, j);
                    }
                }
                deck
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;
    #[test]
    fn cut_0_does_nothing() {
        let start = Deck::new_deck_order();
        let c = start.clone().cut(0);
        assert_eq!(start, c);
    }

    proptest! {
        #[test]
        fn test_cut_twice_roundtrip(cut_pos in 0usize..52) {
            let deck = Deck::new_deck_order();
            let d1 = deck.clone().cut(cut_pos);
            let d2 = d1.cut(52 - cut_pos);
            assert_eq!(deck, d2);

        }
    }
}

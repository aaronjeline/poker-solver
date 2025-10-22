use oorandom::Rand32;

use crate::cards::*;

#[derive(Debug, Clone)]
pub struct Deck(pub Vec<Card>);

impl Deck {
    pub fn apply_mutations(mut self, mutations: impl Iterator<Item = Mutation>) -> Self {
        for mutation in mutations {
            self = self.apply_mutation(mutation);
        }
        self
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

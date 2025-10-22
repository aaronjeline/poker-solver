use crate::deck::*;
use crate::game::*;
use crate::precompute::*;

pub type SearchFn = &'static fn(usize, ScoreTable) -> Deck;

pub fn run_random_search(num_players: usize) -> std::io::Result<()> {
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Loading precomputed hand scores...");
    let f = std::fs::File::open("hands")?;
    let table = load_table(f)?;
    eprintln!("  ✓ Loaded successfully");
    eprintln!();
    eprintln!("  Searching for optimal deck ({} players)...", num_players);
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!();
    let result = genetic_search(num_players, table);
    eprintln!();
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  ✓ Found optimal deck!");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", result);
    Ok(())
}

pub fn run_search(num_players: usize, search: SearchFn) -> std::io::Result<()> {
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Loading precomputed hand scores...");
    let f = std::fs::File::open("hands")?;
    let table = load_table(f)?;
    eprintln!("  ✓ Loaded successfully");
    eprintln!();
    eprintln!("  Searching for optimal deck ({} players)...", num_players);
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!();
    let result = search(num_players, table);
    eprintln!();
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  ✓ Found optimal deck!");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", result);
    Ok(())
}

pub fn genetic_search(num_players: usize, table: ScoreTable) -> Deck {
    const POP_SIZE: usize = 10;
    const NUM_CROSSOVERS: usize = 15; // Number of crossover children to create
    const NUM_MUTATIONS: usize = 15;  // Number of mutations to create
    let start = Deck::new_deck_order();
    let mut rng = oorandom::Rand32::new(4);

    eprintln!("  🧬 Initializing population (size: {})...", POP_SIZE);
    // Initialize the population and evaluate fitness
    let mut scored_population: Vec<(Deck, usize)> = Vec::with_capacity(POP_SIZE);
    for _ in 0..POP_SIZE {
        let deck = start.clone().shuffle(&mut rng);
        let score = num_wins(num_players, &deck, &table);
        scored_population.push((deck, score));
    }

    let initial_best = scored_population.iter().map(|(_, score)| *score).max().unwrap();
    eprintln!("  ✓ Initial population created");
    eprintln!("  📊 Initial best score: {}/{}", initial_best, MAX_WINS);
    eprintln!();

    let mut generation = 0;
    let mut best_score = initial_best;

    loop {
        generation += 1;

        // Extract just the decks for breeding (we'll re-score offspring)
        let population: Vec<Deck> = scored_population.iter().map(|(d, _)| d.clone()).collect();

        // Phase 1: grow the population via crossover and mutation
        let mut new_generation: Vec<(Deck, usize)> = Vec::new();

        // Create children through crossover - select random pairs
        for _ in 0..NUM_CROSSOVERS {
            let i = rng.rand_range(0..population.len() as u32) as usize;
            let j = rng.rand_range(0..population.len() as u32) as usize;
            if i != j {
                let child = Deck::crossover(&population[i], &population[j], &mut rng);
                let score = num_wins(num_players, &child, &table);
                new_generation.push((child, score));
            }
        }

        // Create mutations from random population members
        for _ in 0..NUM_MUTATIONS {
            let i = rng.rand_range(0..population.len() as u32) as usize;
            let muts = generate_mutations(&mut rng);
            let child = population[i].clone().apply_mutations(muts);
            let score = num_wins(num_players, &child, &table);
            new_generation.push((child, score));
        }

        // Add existing population (already scored)
        new_generation.append(&mut scored_population);

        // Sort by fitness (higher is better)
        new_generation.sort_by_key(|(_, score)| *score);
        new_generation.reverse();

        let current_best_score = new_generation[0].1;

        // Print progress when we find improvement
        if current_best_score > best_score {
            best_score = current_best_score;
            eprint!(
                "\r  ⚡ Generation {}: Best score {}/{} (pop: {})",
                generation,
                best_score,
                MAX_WINS,
                new_generation.len()
            );
        } else if generation % 10 == 0 {
            // Print periodic update even without improvement
            eprint!(
                "\r  🔄 Generation {}: Best score {}/{} (pop: {})",
                generation,
                best_score,
                MAX_WINS,
                new_generation.len()
            );
        }

        if current_best_score == MAX_WINS {
            eprintln!();
            eprintln!("  ✓ Perfect deck found after {} generations!", generation);
            return new_generation[0].0.clone();
        }

        // Phase 2: Selection - keep top 50% to maintain diversity
        let survivors = new_generation.len() / 2;
        let survivors = survivors.max(POP_SIZE); // Keep at least POP_SIZE
        new_generation.truncate(survivors);

        scored_population = new_generation;
    }
}

pub fn random_search_for_deck(num_players: usize, table: ScoreTable) -> Deck {
    let start = Deck::new_deck_order();
    let mut random = oorandom::Rand32::new(4);
    let mut iterations = 0;
    let mut best_score = 0;

    loop {
        iterations += 1;
        let shuffled = start.clone().shuffle(&mut random);
        let score = num_wins(num_players, &shuffled, &table);

        if score > best_score {
            best_score = score;
            eprint!(
                "\r  ⚡ Iteration {}: Found deck with score {}/{}",
                iterations, score, MAX_WINS
            );
        }

        if score == MAX_WINS {
            eprintln!();
            eprintln!("  ✓ Perfect deck found after {} iterations!", iterations);
            return shuffled;
        }
    }
}

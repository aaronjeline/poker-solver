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
    const POP_SIZE: usize = 6;
    const NUM_KIDS: usize = 3;
    let start = Deck::new_deck_order();
    let mut rng = oorandom::Rand32::new(4);
    let mut population = Vec::with_capacity(POP_SIZE);

    eprintln!("  🧬 Initializing population (size: {})...", POP_SIZE);
    // Initialize the population
    for _ in 0..POP_SIZE {
        population.push(start.clone().shuffle(&mut rng));
    }

    // Evaluate initial population
    let initial_scores: Vec<usize> = population
        .iter()
        .map(|member| num_wins(num_players, member, &table))
        .collect();
    let initial_best = *initial_scores.iter().max().unwrap();
    eprintln!("  ✓ Initial population created");
    eprintln!("  📊 Initial best score: {}/{}", initial_best, MAX_WINS);
    eprintln!();

    let mut generation = 0;
    let mut best_score = initial_best;

    loop {
        generation += 1;

        // Phase 1: grow the population via crossover and mutation
        let mut new_generation = vec![];

        // Create children through crossover
        for i in 0..population.len() {
            for j in (i + 1)..population.len() {
                // Perform crossover between pairs of parents
                let child = Deck::crossover(&population[i], &population[j], &mut rng);
                new_generation.push(child);
            }
        }

        // Add mutated versions of existing population
        for member in population.iter() {
            for _ in 0..NUM_KIDS {
                let muts = generate_mutations(&mut rng);
                let new = member.clone().apply_mutations(muts);
                new_generation.push(new);
            }
        }

        new_generation.append(&mut population);
        new_generation.sort_by_key(|member| num_wins(num_players, member, &table));
        new_generation.reverse();

        let current_best_score = num_wins(num_players, &new_generation[0], &table);

        // Print progress when we find improvement
        if current_best_score > best_score {
            best_score = current_best_score;
            eprint!(
                "\r  ⚡ Generation {}: Best score {}/{} (pop: {}→{})",
                generation,
                best_score,
                MAX_WINS,
                POP_SIZE,
                new_generation.len()
            );
        } else if generation % 10 == 0 {
            // Print periodic update even without improvement
            eprint!(
                "\r  🔄 Generation {}: Best score {}/{} (pop: {}→{})",
                generation,
                best_score,
                MAX_WINS,
                POP_SIZE,
                new_generation.len()
            );
        }

        if current_best_score == MAX_WINS {
            eprintln!();
            eprintln!("  ✓ Perfect deck found after {} generations!", generation);
            return new_generation[0].clone();
        }

        // Phase 2: cull the population
        new_generation.truncate(2);

        population.push(new_generation[0].clone());
        population.push(new_generation[0].clone());
        population.push(new_generation[0].clone());
        population.push(new_generation[1].clone());
        population.push(new_generation[1].clone());
        population.push(new_generation[1].clone());
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

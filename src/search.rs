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
    let result = simulated_annealing(num_players, table);
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

pub fn hill_climbing(num_players: usize, table: ScoreTable) -> Deck {
    const MAX_SAMPLES: usize = 500; // Limit swaps to test per iteration
    const MAX_RESTARTS: usize = 100; // Maximum random restarts

    let mut rng = oorandom::Rand32::new(4);
    let mut best_ever_deck = Deck::new_deck_order().shuffle(&mut rng);
    let mut best_ever_score = num_wins(num_players, &best_ever_deck, &table);

    eprintln!("  🏔️  Starting hill climbing search...");
    eprintln!("  📊 Initial score: {}/{}", best_ever_score, MAX_WINS);
    eprintln!();

    for restart in 0..MAX_RESTARTS {
        let mut deck = if restart == 0 {
            best_ever_deck.clone()
        } else {
            // Random restart but keep best ever
            Deck::new_deck_order().shuffle(&mut rng)
        };
        let mut current_score = num_wins(num_players, &deck, &table);

        let mut iterations_stuck = 0;
        let mut iteration = 0;

        loop {
            iteration += 1;
            let mut improved = false;

            // Try random swaps instead of exhaustive search
            for _ in 0..MAX_SAMPLES {
                let i = rng.rand_range(0..52) as usize;
                let j = rng.rand_range(0..52) as usize;

                if i == j {
                    continue;
                }

                // Try the swap
                deck.0.swap(i, j);
                let new_score = num_wins(num_players, &deck, &table);

                if new_score > current_score {
                    // Accept improvement
                    current_score = new_score;
                    improved = true;
                    iterations_stuck = 0;

                    if current_score > best_ever_score {
                        best_ever_score = current_score;
                        best_ever_deck = deck.clone();
                        eprint!(
                            "\r  ⚡ Restart {}/{}: Best score {}/{} (iter: {})",
                            restart + 1,
                            MAX_RESTARTS,
                            best_ever_score,
                            MAX_WINS,
                            iteration
                        );
                    }

                    if current_score == MAX_WINS {
                        eprintln!();
                        eprintln!("  ✓ Perfect deck found!");
                        return deck;
                    }

                    break; // Found improvement, try again
                } else {
                    // Reject, swap back
                    deck.0.swap(i, j);
                }
            }

            if !improved {
                iterations_stuck += 1;
                if iterations_stuck > 5 {
                    // Local optimum reached, try random restart
                    if restart % 10 == 0 {
                        eprint!(
                            "\r  🔄 Restart {}/{}: Best score {}/{} (stuck at local optimum)",
                            restart + 1,
                            MAX_RESTARTS,
                            best_ever_score,
                            MAX_WINS
                        );
                    }
                    break; // Move to next restart
                }
            }
        }
    }

    eprintln!();
    eprintln!(
        "  ⚠️  Max restarts reached. Best found: {}/{}",
        best_ever_score, MAX_WINS
    );
    best_ever_deck
}

pub fn genetic_search(num_players: usize, table: ScoreTable) -> Deck {
    const POP_SIZE: usize = 10;
    const ELITE_SIZE: usize = 2; // Top 2 always survive unchanged
    const NUM_CROSSOVERS: usize = 15; // Number of crossover children to create
    const NUM_MUTATIONS: usize = 15; // Number of mutations to create
    const BASE_MUTATION_RATE: f32 = 0.1;
    const HIGH_MUTATION_RATE: f32 = 0.3;
    const STAGNATION_THRESHOLD: usize = 50; // Generations without improvement before boosting mutation

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

    let initial_best = scored_population
        .iter()
        .map(|(_, score)| *score)
        .max()
        .unwrap();
    eprintln!("  ✓ Initial population created");
    eprintln!("  📊 Initial best score: {}/{}", initial_best, MAX_WINS);
    eprintln!();

    let mut generation = 0;
    let mut best_score = initial_best;
    let mut generations_without_improvement = 0;

    loop {
        generation += 1;

        // Adaptive mutation rate based on progress
        let mutation_rate = if generations_without_improvement > STAGNATION_THRESHOLD {
            HIGH_MUTATION_RATE
        } else {
            BASE_MUTATION_RATE
        };

        // Extract just the decks for breeding (we'll re-score offspring)
        let population: Vec<Deck> = scored_population.iter().map(|(d, _)| d.clone()).collect();

        // Phase 1: grow the population via crossover and mutation
        let mut new_generation: Vec<(Deck, usize)> = Vec::new();

        // ELITISM: Preserve the best individuals unchanged
        for i in 0..ELITE_SIZE.min(scored_population.len()) {
            new_generation.push(scored_population[i].clone());
        }

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

        // Create mutations from random population members using advanced mutations
        for _ in 0..NUM_MUTATIONS {
            let i = rng.rand_range(0..population.len() as u32) as usize;
            let muts = generate_adaptive_mutations(&mut rng, mutation_rate);
            let mut child = population[i].clone();
            for mutation in muts {
                child = mutation.apply(child, &mut rng);
            }
            let score = num_wins(num_players, &child, &table);
            new_generation.push((child, score));
        }

        // Add rest of population (already scored, excluding elites which are already added)
        for i in ELITE_SIZE..scored_population.len() {
            new_generation.push(scored_population[i].clone());
        }

        // Sort by fitness (higher is better)
        new_generation.sort_by_key(|(_, score)| *score);
        new_generation.reverse();

        let current_best_score = new_generation[0].1;

        // Track progress for adaptive mutation
        if current_best_score > best_score {
            best_score = current_best_score;
            generations_without_improvement = 0;
            eprint!(
                "\r  ⚡ Generation {}: Best score {}/{} (pop: {}, mut: {:.2})",
                generation,
                best_score,
                MAX_WINS,
                new_generation.len(),
                mutation_rate
            );
        } else {
            generations_without_improvement += 1;
            if generation % 10 == 0 {
                // Print periodic update even without improvement
                eprint!(
                    "\r  🔄 Generation {}: Best score {}/{} (pop: {}, mut: {:.2}, stale: {})",
                    generation,
                    best_score,
                    MAX_WINS,
                    new_generation.len(),
                    mutation_rate,
                    generations_without_improvement
                );
            }
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

pub fn simulated_annealing(num_players: usize, table: ScoreTable) -> Deck {
    const MAX_ITERATIONS: usize = 100_000_000;
    const INITIAL_TEMP: f32 = 10.0;
    const COOLING_RATE: f32 = 0.9999; // Slower cooling = more exploration
    const RESTART_INTERVAL: usize = 50_000; // Restart after this many iterations without improvement
    const MIN_TEMP: f32 = 0.01; // Restart if temperature gets too low

    let mut rng = oorandom::Rand32::new(4);
    let mut best_deck = Deck::new_deck_order().shuffle(&mut rng);
    let mut best_score = num_wins(num_players, &best_deck, &table);

    eprintln!("  🔥 Starting simulated annealing with random restarts...");
    eprintln!("  📊 Initial score: {}/{}", best_score, MAX_WINS);
    eprintln!();

    let mut total_iterations = 0;
    let mut restart_count = 0;

    loop {
        if total_iterations >= MAX_ITERATIONS {
            break;
        }

        restart_count += 1;
        let mut current_deck = if restart_count == 1 {
            best_deck.clone()
        } else {
            // Random restart from new position
            Deck::new_deck_order().shuffle(&mut rng)
        };
        let mut current_score = num_wins(num_players, &current_deck, &table);
        let mut temperature = INITIAL_TEMP;
        let mut iterations_without_improvement = 0;

        while total_iterations < MAX_ITERATIONS {
            total_iterations += 1;

            // Try a random modification using advanced mutations
            let mutation = generate_adaptive_mutations(&mut rng, 0.2)
                .into_iter()
                .next()
                .unwrap();
            let new_deck = mutation.apply(current_deck.clone(), &mut rng);
            let new_score = num_wins(num_players, &new_deck, &table);

            // Calculate acceptance probability
            let accept = if new_score > current_score {
                // Always accept improvements
                true
            } else {
                // Accept worse solutions with probability based on temperature
                let delta = (new_score as f32) - (current_score as f32);
                let probability = (delta / temperature).exp();
                let random_val = rng.rand_float();
                random_val < probability
            };

            if accept {
                current_deck = new_deck;
                current_score = new_score;

                if current_score > best_score {
                    best_score = current_score;
                    best_deck = current_deck.clone();
                    iterations_without_improvement = 0;
                    eprint!(
                        "\r  ⚡ Restart {}, Iter {}: Best score {}/{} (temp: {:.4})",
                        restart_count, total_iterations, best_score, MAX_WINS, temperature
                    );

                    if best_score == MAX_WINS {
                        eprintln!();
                        eprintln!("  ✓ Perfect deck found!");
                        return best_deck;
                    }
                } else {
                    iterations_without_improvement += 1;
                }
            } else {
                iterations_without_improvement += 1;
            }

            // Cool down
            temperature *= COOLING_RATE;

            // Progress update
            if total_iterations % 10000 == 0 {
                eprint!(
                    "\r  🔄 Restart {}, Iter {}: Best {}/{} (current: {}, temp: {:.4})",
                    restart_count, total_iterations, best_score, MAX_WINS, current_score, temperature
                );
            }

            // Check for restart conditions
            if iterations_without_improvement >= RESTART_INTERVAL || temperature < MIN_TEMP {
                eprint!(
                    "\r  🔄 Restart {}: Best {}/{} - Restarting (stuck: {}, temp: {:.4})      ",
                    restart_count, best_score, MAX_WINS, iterations_without_improvement, temperature
                );
                eprintln!();
                break; // Trigger restart
            }
        }
    }

    eprintln!();
    eprintln!(
        "  ⚠️  Max iterations reached after {} restarts. Best found: {}/{}",
        restart_count, best_score, MAX_WINS
    );
    best_deck
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

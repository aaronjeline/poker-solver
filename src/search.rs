use crate::deck::*;
use crate::game::*;
use crate::precompute::*;
use std::sync::Arc;

pub type SearchFn = &'static fn(usize, ScoreTable) -> Deck;

pub const REAL: bool = false;

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
    let mut best_ever_score = num_wins(num_players, &best_ever_deck, &table, REAL);

    eprintln!("  🏔️  Starting hill climbing search...");
    eprintln!("  📊 Initial score: {}/{}", best_ever_score, max_wins(REAL));
    eprintln!();

    for restart in 0..MAX_RESTARTS {
        let mut deck = if restart == 0 {
            best_ever_deck.clone()
        } else {
            // Random restart but keep best ever
            Deck::new_deck_order().shuffle(&mut rng)
        };
        let mut current_score = num_wins(num_players, &deck, &table, REAL);

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
                let new_score = num_wins(num_players, &deck, &table, REAL);

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
                            max_wins(REAL),
                            iteration
                        );
                    }

                    if current_score == max_wins(REAL) {
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
                            max_wins(REAL)
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
        best_ever_score,
        max_wins(REAL)
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
        let score = num_wins(num_players, &deck, &table, REAL);
        scored_population.push((deck, score));
    }

    let initial_best = scored_population
        .iter()
        .map(|(_, score)| *score)
        .max()
        .unwrap();
    eprintln!("  ✓ Initial population created");
    eprintln!(
        "  📊 Initial best score: {}/{}",
        initial_best,
        max_wins(REAL)
    );
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
                let score = num_wins(num_players, &child, &table, REAL);
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
            let score = num_wins(num_players, &child, &table, REAL);
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
                max_wins(REAL),
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
                    max_wins(REAL),
                    new_generation.len(),
                    mutation_rate,
                    generations_without_improvement
                );
            }
        }

        if current_best_score == max_wins(REAL) {
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

fn simulated_annealing_worker(
    num_players: usize,
    table: &ScoreTable,
    thread_id: usize,
    seed: u64,
) -> Deck {
    const INITIAL_TEMP: f32 = 10.0;
    const COOLING_RATE: f32 = 0.9999; // Slower cooling = more exploration
    const BASE_RESTART_INTERVAL: usize = 50_000; // Base restart interval
    const MIN_TEMP: f32 = 0.01; // Restart if temperature gets too low

    let mut rng = oorandom::Rand32::new(seed);
    let mut best_deck = Deck::new_deck_order().shuffle(&mut rng);
    let mut best_score = num_wins(num_players, &best_deck, &table, REAL);

    let mut total_iterations = 0;
    let mut restart_count = 0;

    loop {
        restart_count += 1;

        // Adaptive restart interval: increases with more restarts
        // Early restarts are quick, later ones get more patient
        let restart_interval = BASE_RESTART_INTERVAL * (1 + restart_count / 10);

        let mut current_deck = if restart_count == 1 {
            best_deck.clone()
        } else {
            // Random restart from new position
            Deck::new_deck_order().shuffle(&mut rng)
        };
        let mut current_score = num_wins(num_players, &current_deck, &table, REAL);
        let mut temperature = INITIAL_TEMP;
        let mut iterations_without_improvement = 0;

        loop {
            total_iterations += 1;

            // Try a random modification using advanced mutations
            let mutation = generate_adaptive_mutations(&mut rng, 0.2)
                .into_iter()
                .next()
                .unwrap();
            let new_deck = mutation.apply(current_deck.clone(), &mut rng);
            let new_score = num_wins(num_players, &new_deck, &table, REAL);

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
                        "\r  ⚡ Thread {}, Restart {}, Iter {}: Best score {}/{} (temp: {:.4})",
                        thread_id,
                        restart_count,
                        total_iterations,
                        best_score,
                        max_wins(REAL),
                        temperature
                    );

                    if best_score == max_wins(REAL) {
                        eprintln!();
                        eprintln!("  ✓ Thread {} found perfect deck!", thread_id);
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
                    "\r  🔄 Thread {}, Restart {}, Iter {}: Best {}/{} (current: {}, temp: {:.4})",
                    thread_id,
                    restart_count,
                    total_iterations,
                    best_score,
                    max_wins(REAL),
                    current_score,
                    temperature
                );
            }

            // Check for restart conditions
            if iterations_without_improvement >= restart_interval || temperature < MIN_TEMP {
                if total_iterations % 50000 == 0 {
                    eprint!(
                        "\r  🔄 Thread {}, Restart {}: Best {}/{} - Restarting (stuck: {}, temp: {:.4})      ",
                        thread_id,
                        restart_count,
                        best_score,
                        max_wins(REAL),
                        iterations_without_improvement,
                        temperature,
                    );
                    eprintln!();
                }
                break; // Trigger restart
            }
        }
    }
}

pub fn simulated_annealing(num_players: usize, table: ScoreTable) -> Deck {
    const NUM_THREADS: usize = 10;

    eprintln!("  🔥 Starting parallel simulated annealing with {} threads...", NUM_THREADS);
    eprintln!();

    let table = Arc::new(table);

    // Spawn threads
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let table_clone = Arc::clone(&table);
            let seed = (thread_id as u64) * 1000 + 4; // Different seed for each thread

            std::thread::spawn(move || {
                simulated_annealing_worker(num_players, &table_clone, thread_id, seed)
            })
        })
        .collect();

    // Wait for the first thread to find a perfect solution
    // Use a simple blocking approach with crossbeam's select
    use crossbeam::channel;
    let (tx, rx) = channel::unbounded();

    // Spawn helper threads to wait on each worker and send results
    for (thread_id, handle) in handles.into_iter().enumerate() {
        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            match handle.join() {
                Ok(deck) => {
                    let _ = tx_clone.send((thread_id, deck));
                }
                Err(_) => {
                    eprintln!("  ⚠️  Thread {} panicked", thread_id);
                }
            }
        });
    }
    drop(tx); // Drop the original sender

    // Block until we get the first result
    if let Ok((winning_thread_id, deck)) = rx.recv() {
        eprintln!();
        eprintln!("  🏆 Thread {} won the race!", winning_thread_id);
        deck
    } else {
        eprintln!("  ⚠️  All threads failed");
        Deck::new_deck_order()
    }
}

pub fn analyze_difficulty(num_players: usize, table: ScoreTable, samples: usize) {
    let start = Deck::new_deck_order();
    let mut rng = oorandom::Rand32::new(4);

    let mut scores: Vec<usize> = Vec::new();
    let mut max_seen = 0;

    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Analyzing problem difficulty ({} players)", num_players);
    eprintln!("  Sampling {} random decks...", samples);
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!();

    for i in 0..samples {
        let deck = start.clone().shuffle(&mut rng);
        let score = num_wins(num_players, &deck, &table, REAL);
        scores.push(score);

        if score > max_seen {
            max_seen = score;
            eprint!(
                "\r  New best: {}/{} (sample {}/{})",
                max_seen,
                max_wins(REAL),
                i + 1,
                samples
            );
        } else if i % 100 == 0 {
            eprint!(
                "\r  Progress: {}/{} samples (best: {}/{})",
                i + 1,
                samples,
                max_seen,
                max_wins(REAL)
            );
        }
    }

    eprintln!();
    eprintln!();

    // Calculate statistics
    scores.sort();
    let min = scores[0];
    let max = scores[scores.len() - 1];
    let median = scores[scores.len() / 2];
    let mean: f64 = scores.iter().sum::<usize>() as f64 / scores.len() as f64;

    // Count how many hit certain thresholds
    let perfect = scores.iter().filter(|&&s| s == max_wins(REAL)).count();
    let near_perfect = scores.iter().filter(|&&s| s >= 50).count();
    let good = scores.iter().filter(|&&s| s >= 45).count();
    let decent = scores.iter().filter(|&&s| s >= 40).count();

    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  STATISTICS");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Min score:        {}/{}", min, max_wins(REAL));
    eprintln!("  Max score:        {}/{}", max, max_wins(REAL));
    eprintln!("  Median score:     {}/{}", median, max_wins(REAL));
    eprintln!("  Mean score:       {:.1}/{}", mean, max_wins(REAL));
    eprintln!();
    eprintln!(
        "  Perfect (52/52):  {} ({:.2}%)",
        perfect,
        perfect as f64 / samples as f64 * 100.0
    );
    eprintln!(
        "  ≥50/52:           {} ({:.2}%)",
        near_perfect,
        near_perfect as f64 / samples as f64 * 100.0
    );
    eprintln!(
        "  ≥45/52:           {} ({:.2}%)",
        good,
        good as f64 / samples as f64 * 100.0
    );
    eprintln!(
        "  ≥40/52:           {} ({:.2}%)",
        decent,
        decent as f64 / samples as f64 * 100.0
    );
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!();

    // Distribution by score
    eprintln!("  SCORE DISTRIBUTION");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let mut histogram = vec![0; max_wins(REAL) + 1];
    for &score in &scores {
        histogram[score] += 1;
    }

    for (score, &count) in histogram.iter().enumerate() {
        if count > 0 {
            let bar_len = (count as f64 / samples as f64 * 50.0) as usize;
            let bar = "█".repeat(bar_len);
            eprintln!("  {:2}/52: {:4} {}", score, count, bar);
        }
    }
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}

pub fn random_search_for_deck(num_players: usize, table: ScoreTable) -> Deck {
    let start = Deck::new_deck_order();
    let mut random = oorandom::Rand32::new(4);
    let mut iterations = 0;
    let mut best_score = 0;

    loop {
        iterations += 1;
        let shuffled = start.clone().shuffle(&mut random);
        let score = num_wins(num_players, &shuffled, &table, REAL);

        if score > best_score {
            best_score = score;
            eprint!(
                "\r  ⚡ Iteration {}: Found deck with score {}/{}",
                iterations,
                score,
                max_wins(REAL)
            );
        }

        if score == max_wins(REAL) {
            eprintln!();
            eprintln!("  ✓ Perfect deck found after {} iterations!", iterations);
            return shuffled;
        }
    }
}

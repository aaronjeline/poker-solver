use crate::deck::*;
use crate::game::*;
use crate::precompute::*;
use std::sync::Arc;

pub type SearchFn = fn(usize, ScoreTable) -> Deck;

pub const REAL: bool = false;

/// Calculate Hamming distance between two decks (how many positions differ)
fn hamming_distance(deck1: &Deck, deck2: &Deck) -> usize {
    deck1
        .0
        .iter()
        .zip(deck2.0.iter())
        .filter(|(a, b)| a != b)
        .count()
}

/// Calculate average diversity of a deck compared to all other decks in population
fn calculate_diversity(deck: &Deck, population: &[(Deck, usize)]) -> f32 {
    if population.is_empty() {
        return 0.0;
    }

    let total_distance: usize = population
        .iter()
        .map(|(other_deck, _)| hamming_distance(deck, other_deck))
        .sum();

    total_distance as f32 / population.len() as f32
}

/// Calculate a diversity-adjusted fitness score
/// Rewards both high wins and high diversity from existing population
fn diversity_fitness(score: usize, diversity: f32, diversity_weight: f32) -> f32 {
    score as f32 + diversity_weight * diversity
}

/// Select a parent index using fitness-proportionate (roulette wheel) selection
/// Higher scores have higher probability of being selected
fn select_parent(population: &[(Deck, usize)], rng: &mut oorandom::Rand32) -> usize {
    // Calculate total fitness
    let total_fitness: usize = population.iter().map(|(_, score)| score).sum();

    if total_fitness == 0 {
        // All individuals have 0 fitness, select randomly
        return rng.rand_range(0..population.len() as u32) as usize;
    }

    // Spin the roulette wheel
    let mut spin = rng.rand_range(0..total_fitness as u32) as usize;

    for (idx, (_, score)) in population.iter().enumerate() {
        if spin < *score {
            return idx;
        }
        spin -= score;
    }

    // Fallback (shouldn't reach here due to rounding)
    population.len() - 1
}

/// Perform local search using simulated annealing with hybrid scoring
/// Uses hybrid_score (wins * 100k + margins) internally for smooth gradient
/// Returns (optimized_deck, final_win_count)
fn local_search_sa(
    starting_deck: Deck,
    num_players: usize,
    table: &ScoreTable,
    max_iterations: usize,
    initial_temp: f32,
    cooling_rate: f32,
    rng: &mut oorandom::Rand32,
) -> (Deck, usize) {
    let mut current_deck = starting_deck;
    let mut current_score = hybrid_score(num_players, &current_deck, table, REAL);
    let mut best_deck = current_deck.clone();
    let mut best_score = current_score;
    let mut best_wins = num_wins(num_players, &best_deck, table, REAL);
    let mut temperature = initial_temp;

    for _ in 0..max_iterations {
        // Try a random modification using a single simple mutation
        let mutation = generate_adaptive_mutations(rng, 0.2)
            .into_iter()
            .next()
            .unwrap();
        let new_deck = mutation.apply(current_deck.clone(), rng);
        let new_score = hybrid_score(num_players, &new_deck, table, REAL);

        // Calculate acceptance probability
        let accept = if new_score > current_score {
            // Always accept improvements
            true
        } else {
            // Accept worse solutions with probability based on temperature
            let delta = (new_score - current_score) as f32;
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
                best_wins = num_wins(num_players, &best_deck, table, REAL);

                // Early exit if perfect solution found
                if best_wins == max_wins(REAL) {
                    return (best_deck, best_wins);
                }
            }
        }

        // Cool down
        temperature *= cooling_rate;
    }

    (best_deck, best_wins)
}

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
    const POP_SIZE: usize = 30; // Reduced since SA is expensive per individual
    const ELITE_SIZE: usize = 3; // Top 3 always survive unchanged
    const NUM_CROSSOVERS: usize = 10; // Number of crossover children to create
    const NUM_MUTATIONS: usize = 15; // Number of SA-optimized mutations to create
    const BASE_MUTATION_RATE: f32 = 0.1;
    const HIGH_MUTATION_RATE: f32 = 0.3;
    const STAGNATION_THRESHOLD: usize = 30; // Generations without improvement before boosting mutation
    const MAX_GENERATIONS: usize = 200; // Maximum generations before giving up

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

        // Check generation limit
        if generation > MAX_GENERATIONS {
            eprintln!();
            eprintln!(
                "  ⚠️  Max generations ({}) reached. Best found: {}/{}",
                MAX_GENERATIONS, best_score, max_wins(REAL)
            );
            return scored_population[0].0.clone();
        }

        // Adaptive mutation rate and diversity weight based on progress
        let (mutation_rate, diversity_weight) = if generations_without_improvement > STAGNATION_THRESHOLD {
            // When stuck, use high mutation and high diversity pressure
            (HIGH_MUTATION_RATE, 0.5)
        } else {
            // When progressing, focus more on fitness
            (BASE_MUTATION_RATE, 0.1)
        };

        // Extract just the decks for breeding (we'll re-score offspring)
        let population: Vec<Deck> = scored_population.iter().map(|(d, _)| d.clone()).collect();

        // Phase 1: grow the population via crossover and mutation
        let mut new_generation: Vec<(Deck, usize)> = Vec::new();

        // ELITISM: Preserve the best individuals unchanged
        for i in 0..ELITE_SIZE.min(scored_population.len()) {
            new_generation.push(scored_population[i].clone());
        }

        // Create children through crossover - use fitness-proportionate selection
        for _ in 0..NUM_CROSSOVERS {
            let i = select_parent(&scored_population, &mut rng);
            let j = select_parent(&scored_population, &mut rng);
            if i != j {
                let child = Deck::crossover(&population[i], &population[j], &mut rng);
                let score = num_wins(num_players, &child, &table, REAL);
                new_generation.push((child, score));
            }
        }

        // Create mutations using SA-based local search
        // Adaptive SA budget: low when progressing, high when stagnating
        let sa_iterations = if generations_without_improvement > STAGNATION_THRESHOLD {
            5000 // Deep search when stuck
        } else {
            1000 // Fast search when progressing
        };
        let sa_temp = 5.0;
        let sa_cooling = 0.998;

        for _ in 0..NUM_MUTATIONS {
            // Select parent using fitness-proportionate selection
            let parent_idx = select_parent(&scored_population, &mut rng);
            let parent = &population[parent_idx];

            // Apply 1-2 simple mutations to create starting point
            let mut child = parent.clone();
            let num_initial_mutations = if mutation_rate > 0.2 { 2 } else { 1 };
            for _ in 0..num_initial_mutations {
                let mutation = generate_adaptive_mutations(&mut rng, mutation_rate)
                    .into_iter()
                    .next()
                    .unwrap();
                child = mutation.apply(child, &mut rng);
            }

            // Run local search to optimize
            let (optimized_child, score) = local_search_sa(
                child,
                num_players,
                &table,
                sa_iterations,
                sa_temp,
                sa_cooling,
                &mut rng,
            );
            new_generation.push((optimized_child, score));
        }

        // Add rest of population (already scored, excluding elites which are already added)
        for i in ELITE_SIZE..scored_population.len() {
            new_generation.push(scored_population[i].clone());
        }

        // Sort by diversity-adjusted fitness when stagnating
        if generations_without_improvement > STAGNATION_THRESHOLD / 2 {
            // Calculate diversity for each deck and use diversity-adjusted fitness
            let mut diversity_scored: Vec<_> = new_generation
                .iter()
                .map(|(deck, score)| {
                    let diversity = calculate_diversity(deck, &new_generation);
                    let adjusted_fitness = diversity_fitness(*score, diversity, diversity_weight);
                    (deck.clone(), *score, adjusted_fitness)
                })
                .collect();

            diversity_scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
            new_generation = diversity_scored
                .into_iter()
                .map(|(deck, score, _)| (deck, score))
                .collect();
        } else {
            // Sort by raw fitness (higher is better)
            new_generation.sort_by_key(|(_, score)| *score);
            new_generation.reverse();
        }

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

        // Phase 2: Selection - keep fixed population size
        // This enforces selection pressure by removing worst individuals
        new_generation.truncate(POP_SIZE);

        scored_population = new_generation;
    }
}

/// Evolve a single island
fn evolve_island(
    _island_id: usize,
    mut population: Vec<(Deck, usize)>,
    num_players: usize,
    table: Arc<ScoreTable>,
    generations: usize,
    seed: u64,
) -> Vec<(Deck, usize)> {
    const ISLAND_POP_SIZE: usize = 30;
    const ELITE_SIZE: usize = 3;
    const NUM_CROSSOVERS: usize = 15;
    const NUM_MUTATIONS: usize = 15;
    const BASE_MUTATION_RATE: f32 = 0.1;
    const HIGH_MUTATION_RATE: f32 = 0.3;
    const STAGNATION_THRESHOLD: usize = 30;

    let mut rng = oorandom::Rand32::new(seed);
    let mut stagnation = 0;
    let mut best_score = population[0].1;

    for _ in 0..generations {
        let mutation_rate = if stagnation > STAGNATION_THRESHOLD {
            HIGH_MUTATION_RATE
        } else {
            BASE_MUTATION_RATE
        };

        let population_decks: Vec<Deck> = population.iter().map(|(d, _)| d.clone()).collect();
        let mut new_generation: Vec<(Deck, usize)> = Vec::new();

        // Elitism
        for i in 0..ELITE_SIZE.min(population.len()) {
            new_generation.push(population[i].clone());
        }

        // Crossover with fitness-proportionate selection
        for _ in 0..NUM_CROSSOVERS {
            let i = select_parent(&population, &mut rng);
            let j = select_parent(&population, &mut rng);
            if i != j {
                let child = Deck::crossover(&population_decks[i], &population_decks[j], &mut rng);
                let score = num_wins(num_players, &child, &table, REAL);
                new_generation.push((child, score));
            }
        }

        // Mutation using SA-based local search
        let sa_iterations = if stagnation > STAGNATION_THRESHOLD {
            5000 // Deep search when stuck
        } else {
            1000 // Fast search when progressing
        };
        let sa_temp = 5.0;
        let sa_cooling = 0.998;

        for _ in 0..NUM_MUTATIONS {
            let parent_idx = rng.rand_range(0..population.len() as u32) as usize;
            let parent = &population[parent_idx];

            // Apply 1-2 simple mutations to create starting point
            let mut child = parent.0.clone();
            let num_initial_mutations = if mutation_rate > 0.2 { 2 } else { 1 };
            for _ in 0..num_initial_mutations {
                let mutation = generate_adaptive_mutations(&mut rng, mutation_rate)
                    .into_iter()
                    .next()
                    .unwrap();
                child = mutation.apply(child, &mut rng);
            }

            // Run local search to optimize
            let (optimized_child, score) = local_search_sa(
                child,
                num_players,
                &table,
                sa_iterations,
                sa_temp,
                sa_cooling,
                &mut rng,
            );
            new_generation.push((optimized_child, score));
        }

        // Add rest of population
        for i in ELITE_SIZE..population.len() {
            new_generation.push(population[i].clone());
        }

        // Selection - keep fixed population size
        new_generation.sort_by_key(|(_, score)| *score);
        new_generation.reverse();
        new_generation.truncate(ISLAND_POP_SIZE);

        // Track progress
        let current_best = new_generation[0].1;
        if current_best > best_score {
            best_score = current_best;
            stagnation = 0;
        } else {
            stagnation += 1;
        }

        population = new_generation;

        // Early exit if perfect solution found
        if best_score == max_wins(REAL) {
            break;
        }
    }

    population
}

/// Island model genetic algorithm with multiple isolated populations that occasionally exchange individuals
pub fn island_genetic_search(num_players: usize, table: ScoreTable) -> Deck {
    const NUM_ISLANDS: usize = 10; // One per core
    const ISLAND_POP_SIZE: usize = 30; // Each island has 30 individuals
    const MIGRATION_INTERVAL: usize = 20; // Migrate every 20 generations
    const NUM_MIGRANTS: usize = 2; // Number of individuals to migrate

    let start = Deck::new_deck_order();
    let mut rng = oorandom::Rand32::new(4);
    let table = Arc::new(table);

    eprintln!("  🏝️  Initializing parallel island model ({} islands, {} per island)...", NUM_ISLANDS, ISLAND_POP_SIZE);

    // Initialize islands
    let mut islands: Vec<Vec<(Deck, usize)>> = Vec::with_capacity(NUM_ISLANDS);
    for island_id in 0..NUM_ISLANDS {
        let mut island_pop = Vec::with_capacity(ISLAND_POP_SIZE);
        for _ in 0..ISLAND_POP_SIZE {
            let deck = start.clone().shuffle(&mut rng);
            let score = num_wins(num_players, &deck, &table, REAL);
            island_pop.push((deck, score));
        }
        // Sort by fitness
        island_pop.sort_by_key(|(_, score)| *score);
        island_pop.reverse();

        let best = island_pop[0].1;
        eprintln!("  ✓ Island {} initialized: best {}/{}", island_id, best, max_wins(REAL));
        islands.push(island_pop);
    }
    eprintln!();

    let mut global_best_score = islands.iter()
        .flat_map(|island| island.iter())
        .map(|(_, score)| *score)
        .max()
        .unwrap();

    // Main evolution loop with periodic migration - run forever until solution found
    let mut cycle = 0;
    loop {
        cycle += 1;
        eprintln!("  🔄 Cycle {}: Evolving islands in parallel...", cycle);

        // Evolve each island in parallel for MIGRATION_INTERVAL generations
        let handles: Vec<_> = islands
            .into_iter()
            .enumerate()
            .map(|(island_id, island_pop)| {
                let table_clone = Arc::clone(&table);
                let seed = (island_id as u64) * 1000 + cycle as u64;

                std::thread::spawn(move || {
                    evolve_island(
                        island_id,
                        island_pop,
                        num_players,
                        table_clone,
                        MIGRATION_INTERVAL,
                        seed,
                    )
                })
            })
            .collect();

        // Wait for all islands to finish
        islands = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();

        // Check for perfect solution
        let current_global_best = islands.iter()
            .flat_map(|island| island.iter())
            .map(|(_, score)| *score)
            .max()
            .unwrap();

        if current_global_best > global_best_score {
            global_best_score = current_global_best;
            eprintln!(
                "  ⚡ Cycle {}: New global best {}/{}",
                cycle,
                global_best_score,
                max_wins(REAL)
            );
        } else {
            eprintln!(
                "  📊 Cycle {}: Global best remains {}/{}",
                cycle,
                global_best_score,
                max_wins(REAL)
            );
        }

        if current_global_best == max_wins(REAL) {
            eprintln!();
            eprintln!("  ✓ Perfect deck found after {} cycles!", cycle);
            return islands.iter()
                .flat_map(|island| island.iter())
                .max_by_key(|(_, score)| score)
                .unwrap()
                .0
                .clone();
        }

        // Migration between islands (ring topology)
        eprintln!("  🚢 Migration event...");

        let mut migrants: Vec<Vec<(Deck, usize)>> = Vec::with_capacity(NUM_ISLANDS);
        for island in &islands {
            let mut island_migrants = Vec::new();
            for i in 0..NUM_MIGRANTS.min(island.len()) {
                island_migrants.push(island[i].clone());
            }
            migrants.push(island_migrants);
        }

        // Inject migrants into next island (ring topology)
        for island_id in 0..NUM_ISLANDS {
            let source_island = (island_id + NUM_ISLANDS - 1) % NUM_ISLANDS;

            // Replace worst individuals with migrants from previous island
            for migrant in &migrants[source_island] {
                if islands[island_id].len() > NUM_MIGRANTS {
                    islands[island_id].pop(); // Remove worst
                }
                islands[island_id].push(migrant.clone());
            }

            // Re-sort after migration
            islands[island_id].sort_by_key(|(_, score)| *score);
            islands[island_id].reverse();
        }
        eprintln!();
    }
}

/// Beam search: maintains K diverse high-quality solutions and explores from all of them
pub fn beam_search(num_players: usize, table: ScoreTable) -> Deck {
    const BEAM_WIDTH: usize = 50; // Number of solutions to maintain
    const MUTATIONS_PER_BEAM: usize = 10; // Mutations to generate from each beam member
    const MAX_ITERATIONS: usize = 500;
    const DIVERSITY_WEIGHT: f32 = 0.3; // Weight for diversity in selection
    const SA_ITERATIONS_EARLY: usize = 500; // SA budget early on
    const SA_ITERATIONS_LATE: usize = 2000; // SA budget later when converging

    let start = Deck::new_deck_order();
    let mut rng = oorandom::Rand32::new(4);
    let table = Arc::new(table);

    eprintln!("  🔦 Initializing parallel beam search (beam width: {})...", BEAM_WIDTH);

    // Initialize beam with random decks
    // Store (deck, win_count, hybrid_score) tuples
    let mut beam: Vec<(Deck, usize, f64)> = Vec::with_capacity(BEAM_WIDTH);
    for _ in 0..BEAM_WIDTH {
        let deck = start.clone().shuffle(&mut rng);
        let wins = num_wins(num_players, &deck, &table, REAL);
        let hybrid = hybrid_score(num_players, &deck, &table, REAL);
        beam.push((deck, wins, hybrid));
    }

    // Sort by hybrid score (not just wins!)
    beam.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    let initial_best = beam[0].1;
    eprintln!("  ✓ Initial beam created");
    eprintln!("  📊 Initial best score: {}/{}", initial_best, max_wins(REAL));
    eprintln!();

    let mut best_score = initial_best;
    let mut iterations_without_improvement = 0;

    for iteration in 1..=MAX_ITERATIONS {
        // Adaptive SA budget
        let sa_iterations = if iteration < MAX_ITERATIONS / 4 {
            SA_ITERATIONS_EARLY
        } else {
            SA_ITERATIONS_LATE
        };

        // Generate candidates from all beam members IN PARALLEL
        let mut candidates: Vec<(Deck, usize, f64)> = Vec::new();

        // Keep elite beam members
        for i in 0..BEAM_WIDTH.min(5) {
            candidates.push(beam[i].clone());
        }

        // Parallel mutation generation: spawn a thread for each beam member
        let handles: Vec<_> = beam.iter().enumerate().map(|(beam_idx, (beam_deck, _beam_wins, _beam_hybrid))| {
            let beam_deck = beam_deck.clone();
            let table_clone = Arc::clone(&table);
            let seed = (iteration as u64) * 1000 + (beam_idx as u64);

            std::thread::spawn(move || {
                let mut thread_rng = oorandom::Rand32::new(seed);
                let mut thread_candidates = Vec::with_capacity(MUTATIONS_PER_BEAM);

                for _ in 0..MUTATIONS_PER_BEAM {
                    // Apply 1-2 mutations to create starting point
                    let mut child = beam_deck.clone();
                    let num_mutations = thread_rng.rand_range(1..3) as usize;
                    for _ in 0..num_mutations {
                        let mutation = generate_adaptive_mutations(&mut thread_rng, 0.15)
                            .into_iter()
                            .next()
                            .unwrap();
                        child = mutation.apply(child, &mut thread_rng);
                    }

                    // Run SA local search (returns win count)
                    let (optimized, wins) = local_search_sa(
                        child,
                        num_players,
                        &table_clone,
                        sa_iterations,
                        5.0,
                        0.998,
                        &mut thread_rng,
                    );

                    // Calculate hybrid score for selection
                    let hybrid = hybrid_score(num_players, &optimized, &table_clone, REAL);
                    thread_candidates.push((optimized, wins, hybrid));
                }

                thread_candidates
            })
        }).collect();

        // Collect all candidates from parallel threads
        for handle in handles {
            let thread_candidates = handle.join().unwrap();
            candidates.extend(thread_candidates);
        }

        // Select new beam using hybrid score + diversity
        let mut new_beam: Vec<(Deck, usize, f64)> = Vec::new();

        // Sort candidates by hybrid score
        candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        // Greedily select beam members balancing fitness and diversity
        let mut remaining_candidates = Vec::new();
        for candidate in candidates {
            if new_beam.len() >= BEAM_WIDTH {
                remaining_candidates.push(candidate);
                continue;
            }

            let (cand_deck, cand_wins, cand_hybrid) = &candidate;

            // Calculate diversity from existing beam
            let diversity = if new_beam.is_empty() {
                52.0 // Maximum diversity for first member
            } else {
                // Create temporary vec for diversity calculation
                let temp_beam: Vec<(Deck, usize)> = new_beam.iter()
                    .map(|(d, w, _h)| (d.clone(), *w))
                    .collect();
                calculate_diversity(cand_deck, &temp_beam)
            };

            // Accept if: high fitness OR good diversity
            // Always accept if better than current best wins
            let accept = if *cand_wins >= best_score {
                true // Always accept improvements
            } else {
                // Use diversity-adjusted hybrid fitness
                let adjusted_fitness = *cand_hybrid + (DIVERSITY_WEIGHT * diversity) as f64;
                let best_hybrid = beam[0].2;
                let threshold = best_hybrid - 500_000.0; // Within reasonable range
                adjusted_fitness >= threshold
            };

            if accept {
                new_beam.push(candidate);
            } else {
                remaining_candidates.push(candidate);
            }
        }

        // If beam is too small, fill with best remaining candidates regardless of diversity
        if new_beam.len() < BEAM_WIDTH {
            remaining_candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
            for candidate in remaining_candidates {
                if new_beam.len() >= BEAM_WIDTH {
                    break;
                }
                if !new_beam.iter().any(|(d, _, _)| d == &candidate.0) {
                    new_beam.push(candidate);
                }
            }
        }

        beam = new_beam;

        // Sort beam by hybrid score
        beam.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        let current_best = beam[0].1;

        // Track progress
        if current_best > best_score {
            best_score = current_best;
            iterations_without_improvement = 0;
            eprint!(
                "\r  ⚡ Iteration {}/{}: Best score {}/{} (beam: {}, SA: {})",
                iteration,
                MAX_ITERATIONS,
                best_score,
                max_wins(REAL),
                beam.len(),
                sa_iterations
            );
        } else {
            iterations_without_improvement += 1;
            if iteration % 10 == 0 {
                eprint!(
                    "\r  🔄 Iteration {}/{}: Best score {}/{} (stale: {}, SA: {})",
                    iteration,
                    MAX_ITERATIONS,
                    best_score,
                    max_wins(REAL),
                    iterations_without_improvement,
                    sa_iterations
                );
            }
        }

        // Check for perfect solution
        if current_best == max_wins(REAL) {
            eprintln!();
            eprintln!("  ✓ Perfect deck found after {} iterations!", iteration);
            return beam[0].0.clone();
        }
    }

    eprintln!();
    eprintln!(
        "  ⚠️  Max iterations reached. Best found: {}/{}",
        best_score,
        max_wins(REAL)
    );
    beam[0].0.clone()
}

/// Calculate heuristic value for placing a card at a position
/// Enhanced with multiple factors: card strength, position frequency, suit diversity
fn calculate_heuristic(position: usize, card: u8, num_players: usize, deck_so_far: &[u8]) -> f32 {
    // 1. Card strength: Aces=13, Kings=12, ..., 2s=1
    let card_value = (card % 13) + 1;
    let card_strength = if card_value >= 10 {
        // Face cards and aces are significantly more valuable
        card_value as f32 * 1.5
    } else {
        card_value as f32
    };

    // 2. Count how many cuts result in dealer getting this position
    let mut dealer_gets_position = 0;
    let mut dealer_hole_cards = 0; // First 2 cards dealer gets
    let mut common_cards = 0; // Cards that go to the board

    let cut_range = if REAL { 5..=47 } else { 0..=51 };
    for cut_pos in cut_range {
        let dealing_position = (position + 52 - cut_pos) % 52;
        let player_who_gets_it = dealing_position % num_players;

        if player_who_gets_it == 0 {
            dealer_gets_position += 1;

            // Check if this is a hole card (first 2*num_players cards dealt)
            if dealing_position < 2 * num_players {
                dealer_hole_cards += 1;
            }
        }

        // Check if this goes to the common cards (next 5 cards after hole cards)
        if dealing_position >= 2 * num_players && dealing_position < 2 * num_players + 5 {
            common_cards += 1;
        }
    }

    // 3. Suit diversity bonus
    let card_suit = card / 13;
    let mut suit_counts = [0u32; 4];
    for &placed_card in deck_so_far {
        suit_counts[(placed_card / 13) as usize] += 1;
    }

    // Bonus for balancing suits (prevents all one suit)
    let current_suit_count = suit_counts[card_suit as usize];
    let target_suit_count = deck_so_far.len() / 4;
    let suit_balance = if current_suit_count < target_suit_count as u32 {
        1.2 // Bonus for underrepresented suits
    } else if current_suit_count > target_suit_count as u32 + 3 {
        0.8 // Penalty for overrepresented suits
    } else {
        1.0
    };

    // 4. Combined heuristic
    // Hole cards are more important than common cards
    let position_value = (dealer_hole_cards as f32 * 3.0) +
                        (dealer_gets_position as f32 * 1.5) +
                        (common_cards as f32 * 0.5);

    card_strength * position_value * suit_balance
}

/// Build a deck constructively using pheromone trails and heuristic
fn build_deck_constructively(
    pheromone: &[[f32; 52]; 52],
    num_players: usize,
    alpha: f32,
    beta: f32,
    rng: &mut oorandom::Rand32,
) -> Deck {
    let mut available_cards: Vec<u8> = (0..52).collect();
    let mut deck_cards = Vec::with_capacity(52);
    let mut deck_cards_u8: Vec<u8> = Vec::with_capacity(52); // For heuristic calculation

    for position in 0..52 {
        // Calculate probabilities for each available card
        let mut probabilities: Vec<(u8, f32)> = Vec::new();
        let mut total_prob = 0.0;

        for &card in &available_cards {
            let tau = pheromone[position][card as usize];
            let eta = calculate_heuristic(position, card, num_players, &deck_cards_u8);
            let prob = tau.powf(alpha) * eta.powf(beta);
            probabilities.push((card, prob));
            total_prob += prob;
        }

        // Normalize probabilities
        if total_prob > 0.0 {
            for (_, p) in &mut probabilities {
                *p /= total_prob;
            }
        } else {
            // If all probabilities are 0, use uniform
            let uniform = 1.0 / available_cards.len() as f32;
            for (_, p) in &mut probabilities {
                *p = uniform;
            }
        }

        // Select card using roulette wheel selection
        let spin = rng.rand_float();
        let mut cumulative = 0.0;
        let mut selected_card = available_cards[0];

        for (card, prob) in probabilities {
            cumulative += prob;
            if spin <= cumulative {
                selected_card = card;
                break;
            }
        }

        // Add to deck and remove from available
        deck_cards.push(crate::cards::Card(selected_card));
        deck_cards_u8.push(selected_card);
        available_cards.retain(|&c| c != selected_card);
    }

    Deck(deck_cards)
}

/// Ant Colony Optimization: builds decks constructively with pheromone guidance
pub fn ant_colony_search(num_players: usize, table: ScoreTable) -> Deck {
    const NUM_ANTS: usize = 30;
    const MAX_ITERATIONS: usize = 500;
    const ALPHA: f32 = 1.0; // Pheromone weight
    const BETA: f32 = 2.0; // Heuristic weight (favor heuristic initially)
    const RHO: f32 = 0.1; // Evaporation rate
    const ELITE_ANTS: usize = 5; // Top ants that deposit pheromone
    const SA_ITERATIONS: usize = 500; // SA refinement budget
    const RESTART_THRESHOLD: usize = 50; // Restart if stuck for this many iterations
    const MAX_RESTARTS: usize = 10; // Maximum number of restarts

    let mut rng = oorandom::Rand32::new(4);

    eprintln!("  🐜 Initializing Ant Colony Optimization...");
    eprintln!("     Ants: {}, Iterations per restart: {}", NUM_ANTS, MAX_ITERATIONS);
    eprintln!("     α={} (pheromone), β={} (heuristic), ρ={} (evaporation)", ALPHA, BETA, RHO);
    eprintln!("     Restart threshold: {} iterations", RESTART_THRESHOLD);
    eprintln!();

    let mut best_ever_deck = Deck::new_deck_order();
    let mut best_ever_score = 0;
    let mut restart_count = 0;

    while restart_count < MAX_RESTARTS {
        restart_count += 1;

        eprintln!("  🔄 Restart {}/{}: Resetting pheromones...", restart_count, MAX_RESTARTS);

        // Initialize/reset pheromone matrix (all neutral)
        let mut pheromone = [[1.0f32; 52]; 52];
        let mut iterations_without_improvement = 0;

    for iteration in 1..=MAX_ITERATIONS {
        // Build phase: each ant constructs a deck
        let mut ants: Vec<(Deck, usize)> = Vec::with_capacity(NUM_ANTS);

        for _ in 0..NUM_ANTS {
            let deck = build_deck_constructively(&pheromone, num_players, ALPHA, BETA, &mut rng);

            // Optional: Apply SA refinement
            let (refined_deck, score) = local_search_sa(
                deck,
                num_players,
                &table,
                SA_ITERATIONS,
                5.0,
                0.998,
                &mut rng,
            );

            ants.push((refined_deck, score));
        }

        // Sort ants by fitness
        ants.sort_by_key(|(_, score)| *score);
        ants.reverse();

        let iteration_best_score = ants[0].1;

        // Track global best
        if iteration_best_score > best_ever_score {
            best_ever_score = iteration_best_score;
            best_ever_deck = ants[0].0.clone();
            iterations_without_improvement = 0;
            eprint!(
                "\r  ⚡ Restart {}, Iter {}: Best {}/{} (α={}, β={})",
                restart_count,
                iteration,
                best_ever_score,
                max_wins(REAL),
                ALPHA,
                BETA
            );
        } else {
            iterations_without_improvement += 1;
            if iteration % 10 == 0 {
                eprint!(
                    "\r  🔄 Restart {}, Iter {}: Best {}/{} (stale: {})",
                    restart_count,
                    iteration,
                    best_ever_score,
                    max_wins(REAL),
                    iterations_without_improvement
                );
            }
        }

        // Check for perfect solution
        if best_ever_score == max_wins(REAL) {
            eprintln!();
            eprintln!("  ✓ Perfect deck found after restart {}, iteration {}!", restart_count, iteration);
            return best_ever_deck;
        }

        // Check for restart condition
        if iterations_without_improvement >= RESTART_THRESHOLD {
            eprintln!();
            eprintln!("  ⚠️  Stuck at {}/{} for {} iterations. Triggering restart...",
                     best_ever_score, max_wins(REAL), RESTART_THRESHOLD);
            break; // Break inner loop, continue to next restart
        }

        // Pheromone update phase

        // 1. Evaporation
        for pos in 0..52 {
            for card in 0..52 {
                pheromone[pos][card] *= 1.0 - RHO;
            }
        }

        // 2. Deposit from elite ants
        for i in 0..ELITE_ANTS.min(ants.len()) {
            let (deck, score) = &ants[i];
            let deposit_amount = (*score as f32) / (max_wins(REAL) as f32);

            for (position, card) in deck.0.iter().enumerate() {
                pheromone[position][card.0 as usize] += deposit_amount;
            }
        }
    }
    } // End restart while loop

    eprintln!();
    eprintln!(
        "  ⚠️  Max restarts ({}) reached. Best found: {}/{}",
        MAX_RESTARTS,
        best_ever_score,
        max_wins(REAL)
    );
    best_ever_deck
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

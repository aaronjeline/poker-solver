mod cards;
mod deck;
mod game;
mod hands;
mod precompute;
mod search;
mod viz;

use clap::{Parser, Subcommand};
use std::io::{self, stdout};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "poker_wins")]
#[command(about = "Poker hand analysis tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Precompute poker hand lookup table
    Precompute,
    /// Search for optimal deck configuration
    Search {
        /// Number of players (including dealer)
        #[arg(short, long, default_value = "2")]
        num_players: usize,
        /// Search algorithm to use: genetic, island, beam, aco, simulated-annealing, hill-climbing
        #[arg(short, long, default_value = "genetic")]
        algorithm: String,
    },
    /// Analyze problem difficulty for given player count
    Analyze {
        /// Number of players (including dealer)
        #[arg(short, long, default_value = "2")]
        num_players: usize,
        /// Number of random samples to test
        #[arg(short, long, default_value = "10000")]
        samples: usize,
    },
    /// Export an interactive fitness-landscape visualization as a self-contained HTML file
    Viz {
        /// Output HTML file path
        #[arg(short, long, default_value = "landscape.html")]
        output: PathBuf,
        /// Number of random-restart climbs to run per player count
        #[arg(short, long, default_value = "60")]
        restarts: usize,
        /// Comma-separated player counts to analyze
        #[arg(long, default_value = "2,3,4")]
        players: String,
        /// RNG seed
        #[arg(long, default_value = "4")]
        seed: u64,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Precompute => {
            precompute::precompute(stdout())?;
        }
        Commands::Search { num_players, algorithm } => {
            let search_fn: search::SearchFn = match algorithm.as_str() {
                "genetic" => search::genetic_search,
                "island" => search::island_genetic_search,
                "beam" => search::beam_search,
                "aco" => search::ant_colony_search,
                "simulated-annealing" => search::simulated_annealing,
                "hill-climbing" | "hill" => search::hill_climbing,
                _ => {
                    eprintln!("Unknown algorithm '{}'. Using genetic search.", algorithm);
                    search::genetic_search
                }
            };
            search::run_search(num_players, search_fn)?;
        }
        Commands::Analyze { num_players, samples } => {
            let f = std::fs::File::open("hands")?;
            let table = precompute::load_table(f)?;
            search::analyze_difficulty(num_players, table, samples);
        }
        Commands::Viz { output, restarts, players, seed } => {
            let player_counts: Vec<usize> = players
                .split(',')
                .map(|s| s.trim().parse().expect("invalid player count"))
                .collect();
            let f = std::fs::File::open("hands")?;
            let table = precompute::load_table(f)?;
            viz::export(&table, &player_counts, restarts, seed, &output)?;
        }
    }

    Ok(())
}

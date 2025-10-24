mod cards;
mod deck;
mod game;
mod hands;
mod precompute;
mod search;

use clap::{Parser, Subcommand};
use std::io::{self, stdout};

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
        /// Search algorithm to use: genetic, island, beam, aco, simulated-annealing
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
    }

    Ok(())
}

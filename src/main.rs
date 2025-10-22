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
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Precompute => {
            precompute::precompute(stdout())?;
        }
        Commands::Search { num_players } => {
            search::run_random_search(num_players)?;
        }
    }

    Ok(())
}

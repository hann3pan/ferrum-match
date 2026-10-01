mod modes;

use clap::{Parser, Subcommand};

use crate::modes::interactive::interactive_mode_loop;

#[derive(Parser, Debug)]
#[command(
    name = "ferrum",
    version,
    about = "Ferrum crypto exchange matching engine",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<CommandModeCommands>,
}

#[derive(Subcommand, Debug)]
enum CommandModeCommands {
    /// Start an interactive REPL backed by a single in-memory order book.
    Interactive,
    /// Headless mode (not implemented yet).
    Run {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

fn main() -> rustyline::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(CommandModeCommands::Interactive) => {
            interactive_mode_loop()?;
        }
        Some(CommandModeCommands::Run { port }) => {
            eprintln!("ferrum run: headless mode on port {port} is not implemented yet");
            std::process::exit(1);
        }
        None => {
            println!("No command given. Run `ferrum --help` for usage.");
        }
    }
    Ok(())
}

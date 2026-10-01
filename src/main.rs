mod modes;

use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};
use std::ops::ControlFlow;
use tracing::info;

use crate::modes::interactive::{interactive_mode, parse_price, parse_quantity};
use clap::{Parser, Subcommand};
use ferrum_match::orderbook::types::{OrderBook, OrderId, Side};

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
    Interactive,
    Add {
        #[arg(short, long)]
        side: String,
        #[arg(short, long)]
        price: String,
        #[arg(short, long)]
        quantity: String,
    },
    Cancel {
        #[arg(short, long)]
        id: u64,
    },
    ViewBook,
    Run {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(CommandModeCommands::Interactive) => {
            interactive_mode_loop()?;
        }
        Some(CommandModeCommands::Add {
            side,
            price,
            quantity,
        }) => {
            add(side, price, quantity)?;
        }
        Some(CommandModeCommands::Cancel { id }) => {
            cancel(id);
        }
        Some(CommandModeCommands::ViewBook) => {
            print_book();
        }
        Some(CommandModeCommands::Run { port }) => {
            println!("Starting headless mode on port {port} (not yet implemented)");
        }
        None => {
            println!("No command given. Run `ferrum --help` for usage.");
        }
    }
    Ok(())
}

fn print_book() {
    let orderbook = OrderBook::new();

    println!("best bid: {:?}", orderbook.best_bid());
    println!("best ask: {:?}", orderbook.best_ask());
}

fn cancel(id: u64) {
    let mut orderbook = OrderBook::new();

    orderbook.cancel_order(OrderId(id));
}

fn add(side: String, price: String, quantity: String) -> Result<()> {
    info!("COMMAND MODE: Matching engine started");
    println!(
        "COMMAND MODE: Received arguments are: side={}, price={}, quantity={}",
        side, price, quantity
    );

    let mut orderbook = OrderBook::new();

    let p = parse_price(&price.as_str()).unwrap_or(0);
    let q = parse_quantity(&quantity.as_str()).unwrap_or(0);

    let side_lower_case = side.to_lowercase();
    let s = parse_side(side_lower_case).unwrap();

    info!("COMMAND MODE: Created empty orderbook");

    match orderbook.matching_order(OrderBook::make_order_request(p, q, s)) {
        Ok(trades) => println!("Matched {} trades", trades.len()),
        Err(e) => println!("Order rejected: {:?}", e),
    }
    Ok(())
}

fn parse_side(side: String) -> std::result::Result<Side, String> {
    let side: &&str = &side.as_str();
    match *side {
        "buy" => Ok(Side::Bid),
        "sell" => Ok(Side::Ask),
        _ => Err("Invalid side: use BUY or SELL".to_string()),
    }
}

fn interactive_mode_loop() -> Result<()> {
    let mut orderbook = OrderBook::new();

    let mut rl = DefaultEditor::new()?;

    Ok(loop {
        let readline = rl.readline("ferrum> ");
        match readline {
            Ok(line) => {
                if let ControlFlow::Break(_) = interactive_mode(&mut orderbook, line.as_str()) {
                    break;
                }

                rl.add_history_entry(line.as_str())?;
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    })
}

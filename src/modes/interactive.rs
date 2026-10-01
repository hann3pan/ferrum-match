use std::ops::ControlFlow;

use clap::ValueEnum;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use ferrum_match::orderbook::types::{OrderBook, OrderError, OrderId, Side};

#[derive(Debug, PartialEq)]
enum Command {
    Submit { side: Side, price: u64, qty: u64 },
    Cancel { id: u64 },
    Print,
    Exit,
}

/// Mirrors `Side`, but gives REPL side tokens clap's `ValueEnum` parsing
/// (case-insensitive matching, one source of truth for accepted spellings)
/// instead of a hand-rolled string match.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum SideArg {
    Buy,
    Sell,
}

impl From<SideArg> for Side {
    fn from(value: SideArg) -> Self {
        match value {
            SideArg::Buy => Side::Bid,
            SideArg::Sell => Side::Ask,
        }
    }
}

fn parse_command_or_error(input: &str) -> Result<Command, String> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    match parts.as_slice() {
        ["print"] => Ok(Command::Print),
        [side, price, qty] => {
            let p = parse_price(price)?;
            let q = parse_quantity(qty)?;

            if p == 0 || q == 0 {
                return Err("Price and quantity must be > 0".to_string());
            }

            parse_side(side, p, q)
        }
        ["delete", id] => Ok(Command::Cancel { id: parse_id(id)? }),
        ["exit"] => Ok(Command::Exit),
        _ => Err("Parsing failed".to_string()),
    }
}

fn parse_side(side: &str, p: u64, q: u64) -> Result<Command, String> {
    let side_arg =
        SideArg::from_str(side, true).map_err(|_| "Invalid side: use BUY or SELL".to_string())?;
    Ok(Command::Submit {
        side: side_arg.into(),
        price: p,
        qty: q,
    })
}

pub fn parse_quantity(qty: &str) -> Result<u64, String> {
    let q = qty.parse::<u64>().map_err(|_| "Invalid quantity")?;
    Ok(q)
}

pub fn parse_price(price: &str) -> Result<u64, String> {
    let p = price.parse::<u64>().map_err(|_| "Invalid price")?;
    Ok(p)
}

pub fn parse_id(id: &str) -> Result<u64, String> {
    let result = id.parse::<u64>().map_err(|_| "Invalid id")?;
    Ok(result)
}

/// Readable text for a rejected order. Kept here rather than in
/// `src/orderbook` so the domain crate stays free of presentation concerns.
fn describe_order_error(err: OrderError) -> &'static str {
    match err {
        OrderError::ZeroPrice => "Order rejected: price must be greater than zero",
        OrderError::ZeroQuantity => "Order rejected: quantity must be greater than zero",
    }
}

/// Renders the full book (asks, then bids) via `OrderBook::levels`, best
/// price first on each side. Formatting lives here rather than in
/// `src/orderbook` so the domain crate stays free of presentation concerns.
fn format_book(orderbook: &OrderBook) -> String {
    let mut output = String::new();
    format_side(&mut output, "Asks", orderbook, Side::Ask);
    format_side(&mut output, "Bids", orderbook, Side::Bid);
    output
}

fn format_side(output: &mut String, label: &str, orderbook: &OrderBook, side: Side) {
    output.push_str(label);
    output.push('\n');
    for (price, orders) in orderbook.levels(side) {
        output.push_str(&format!("  {price}\n"));
        for order in orders {
            output.push_str(&format!(
                "    id={} qty={} arrival_seq={}\n",
                order.id.0, order.quantity, order.arrival_seq
            ));
        }
    }
}

pub fn interactive_mode(orderbook: &mut OrderBook, line: &str) -> ControlFlow<()> {
    match parse_command_or_error(&line.to_lowercase()) {
        Ok(cmd) => match cmd {
            Command::Print => {
                print!("{}", format_book(orderbook));
            }
            Command::Submit { side, price, qty } => {
                match orderbook.matching_order(OrderBook::make_order_request(price, qty, side)) {
                    Ok(trades) => println!(
                        "Successfully made {} trades, trade information:\n{:?}",
                        trades.len(),
                        trades
                    ),
                    Err(e) => println!("{}", describe_order_error(e)),
                }
            }
            Command::Cancel { id } => match orderbook.cancel_order(OrderId(id)) {
                Some(_) => println!("Successfully cancelled order with id: {}", id),
                None => println!(
                    "Failed to cancel order. Order with order id {} is not found.",
                    id
                ),
            },
            Command::Exit => {
                println!("EXIT");
                return ControlFlow::Break(());
            }
        },
        Err(e) => println!("{}", e),
    }
    ControlFlow::Continue(())
}

/// Runs the REPL: reads a line, dispatches it against a single in-memory
/// order book, and repeats until `exit`, EOF or an interrupt.
pub fn interactive_mode_loop() -> rustyline::Result<()> {
    let mut orderbook = OrderBook::new();
    let mut rl = DefaultEditor::new()?;

    loop {
        let readline = rl.readline("ferrum> ");
        match readline {
            Ok(line) => {
                if let ControlFlow::Break(_) = interactive_mode(&mut orderbook, &line) {
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
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_orderbook() -> OrderBook {
        OrderBook::new()
    }

    #[test]
    fn parse_print() {
        assert!(matches!(
            parse_command_or_error("print"),
            Ok(Command::Print)
        ));
    }

    #[test]
    fn parse_exit() {
        assert!(matches!(parse_command_or_error("exit"), Ok(Command::Exit)));
    }

    #[test]
    fn parse_buy() {
        let cmd = parse_command_or_error("buy 10 5").unwrap();
        assert!(matches!(
            cmd,
            Command::Submit {
                side: Side::Bid,
                price: 10,
                qty: 5
            }
        ));
    }

    #[test]
    fn parse_sell() {
        let cmd = parse_command_or_error("sell 99 1").unwrap();
        assert!(matches!(
            cmd,
            Command::Submit {
                side: Side::Ask,
                price: 99,
                qty: 1
            }
        ));
    }

    #[test]
    fn parse_trims_whitespace() {
        let cmd = parse_command_or_error("  buy  7  3  ").unwrap();
        assert!(matches!(
            cmd,
            Command::Submit {
                side: Side::Bid,
                price: 7,
                qty: 3
            }
        ));
    }

    #[test]
    fn parse_invalid_price() {
        assert_eq!(
            parse_command_or_error("buy x 1"),
            Err("Invalid price".to_string())
        );
    }

    #[test]
    fn parse_invalid_quantity() {
        assert_eq!(
            parse_command_or_error("sell 1 y"),
            Err("Invalid quantity".to_string())
        );
    }

    #[test]
    fn parse_zero_price() {
        assert_eq!(
            parse_command_or_error("buy 0 1"),
            Err("Price and quantity must be > 0".to_string())
        );
    }

    #[test]
    fn parse_zero_quantity() {
        assert_eq!(
            parse_command_or_error("sell 1 0"),
            Err("Price and quantity must be > 0".to_string())
        );
    }

    #[test]
    fn parse_invalid_side() {
        assert_eq!(
            parse_command_or_error("hold 1 1"),
            Err("Invalid side: use BUY or SELL".to_string())
        );
    }

    #[test]
    fn parse_wrong_arity() {
        assert_eq!(
            parse_command_or_error("buy 1"),
            Err("Parsing failed".to_string())
        );
    }

    #[test]
    fn parse_empty_input() {
        assert_eq!(
            parse_command_or_error(""),
            Err("Parsing failed".to_string())
        );
    }

    #[test]
    fn parse_gibberish() {
        assert_eq!(
            parse_command_or_error("hello world"),
            Err("Parsing failed".to_string())
        );
    }

    #[test]
    fn parse_delete_invalid_id() {
        assert_eq!(
            parse_command_or_error("delete abc"),
            Err("Invalid id".to_string())
        );
    }

    #[test]
    fn parse_delete_missing_id() {
        assert_eq!(
            parse_command_or_error("delete"),
            Err("Parsing failed".to_string())
        );
    }

    #[test]
    fn parse_price_overflows_u64() {
        assert_eq!(
            parse_command_or_error("buy 99999999999999999999 1"),
            Err("Invalid price".to_string())
        );
    }

    #[test]
    fn parse_negative_price() {
        assert_eq!(
            parse_command_or_error("buy -1 1"),
            Err("Invalid price".to_string())
        );
    }

    #[test]
    fn parse_negative_quantity() {
        assert_eq!(
            parse_command_or_error("buy 1 -1"),
            Err("Invalid quantity".to_string())
        );
    }

    #[test]
    fn interactive_exit_breaks() {
        let mut ob = empty_orderbook();
        let flow = interactive_mode(&mut ob, "exit");
        assert!(flow.is_break());
    }

    #[test]
    fn interactive_invalid_returns_continue() {
        let mut ob = empty_orderbook();
        let flow = interactive_mode(&mut ob, "not-a-command");
        assert!(flow.is_continue());
    }

    #[test]
    fn interactive_buy_resting_on_book() {
        let mut ob = empty_orderbook();
        let flow = interactive_mode(&mut ob, "buy 100 3");
        assert!(flow.is_continue());
        let level = ob.orders_at(Side::Bid, 100).expect("bid level");
        assert_eq!(level.len(), 1);
        assert_eq!(level[0].quantity, 3);
    }

    #[test]
    fn interactive_sell_resting_on_book() {
        let mut ob = empty_orderbook();
        let flow = interactive_mode(&mut ob, "sell 50 2");
        assert!(flow.is_continue());
        let level = ob.orders_at(Side::Ask, 50).expect("ask level");
        assert_eq!(level.len(), 1);
        assert_eq!(level[0].quantity, 2);
    }

    #[test]
    fn print_shows_full_book_best_price_first_per_side() {
        let mut ob = empty_orderbook();
        let _ = interactive_mode(&mut ob, "buy 100 5");
        let _ = interactive_mode(&mut ob, "buy 100 3");
        let _ = interactive_mode(&mut ob, "buy 99 1");
        let _ = interactive_mode(&mut ob, "sell 150 2");

        let output = format_book(&ob);

        // Each order's id, quantity and arrival_seq show up.
        assert!(output.contains("id=1 qty=5 arrival_seq=0"));
        assert!(output.contains("id=2 qty=3 arrival_seq=1"));
        assert!(output.contains("id=3 qty=1 arrival_seq=2"));
        assert!(output.contains("id=4 qty=2 arrival_seq=3"));

        // Bids are best price (100) first, i.e. before the 99 level.
        let pos_100 = output.find("  100\n").expect("bid level 100");
        let pos_99 = output.find("  99\n").expect("bid level 99");
        assert!(pos_100 < pos_99, "bid levels must be best-price-first");

        // Asks section comes before bids section, per format_book's order.
        let pos_asks = output.find("Asks").expect("asks header");
        let pos_bids = output.find("Bids").expect("bids header");
        assert!(pos_asks < pos_bids);
    }

    #[test]
    fn interactive_case_insensitive_side() {
        let mut ob = empty_orderbook();
        assert!(interactive_mode(&mut ob, "BUY 10 1").is_continue());
        assert!(ob.orders_at(Side::Bid, 10).is_some());
    }

    #[test]
    fn interactive_delete_invalid_id_keeps_looping() {
        let mut ob = empty_orderbook();
        assert!(interactive_mode(&mut ob, "delete abc").is_continue());
    }

    #[test]
    fn interactive_delete_missing_id_keeps_looping() {
        let mut ob = empty_orderbook();
        assert!(interactive_mode(&mut ob, "delete").is_continue());
    }

    #[test]
    fn interactive_overflow_price_keeps_looping() {
        let mut ob = empty_orderbook();
        assert!(interactive_mode(&mut ob, "buy 99999999999999999999 1").is_continue());
    }

    #[test]
    fn interactive_negative_price_keeps_looping() {
        let mut ob = empty_orderbook();
        assert!(interactive_mode(&mut ob, "buy -1 1").is_continue());
    }

    #[test]
    fn interactive_zero_price_keeps_looping() {
        let mut ob = empty_orderbook();
        assert!(interactive_mode(&mut ob, "buy 0 1").is_continue());
    }

    #[test]
    fn interactive_zero_quantity_keeps_looping() {
        let mut ob = empty_orderbook();
        assert!(interactive_mode(&mut ob, "sell 1 0").is_continue());
    }
}

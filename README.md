# ferrum-match

A small matching engine for a crypto exchange. You place buy and sell orders; when prices cross, they trade. Anything left over stays on the order book.

## Stack

- **Language:** Rust (2021 edition)
- **CLI:** [clap](https://crates.io/crates/clap) for commands, [rustyline](https://crates.io/crates/rustyline) for the interactive prompt
- **Logging:** [tracing](https://crates.io/crates/tracing)
- **Tests:** `cargo test`, plus [proptest](https://crates.io/crates/proptest) for property-based tests
- **CI:** GitHub Actions

## How to use

Install [Rust](https://rustup.rs/), then from this repo:

```bash
cargo run -- --help
```

### Interactive mode

Best way to try it. One order book stays in memory while you type commands:

```bash
cargo run -- interactive
```

| Command | What it does |
| --- | --- |
| `buy 100 5` | Buy 5 at price 100 |
| `sell 100 3` | Sell 3 at price 100 |
| `print` | Show the book |
| `delete 1` | Cancel order id 1 |
| `exit` | Quit |

Price and quantity are whole numbers. If a buy and a sell overlap, they match. Leftover size stays on the book.

Bad input (an invalid id, a non-numeric price, a price/quantity of zero, ...) prints an error and
keeps the prompt running instead of crashing.

### Headless mode

`cargo run -- run` is not implemented yet; it prints a message and exits with a non-zero status.

### Tests

```bash
cargo test
```

## High-Over View (system architecture)
```mermaid
graph TD
    A[User/Client] -->|API Request| B(Gateway Layer)
    B -->|Validate Auth| C{Risk Engine}
    C -->|Insufficient Funds| D[Reject Order]
    C -->|Valid| E[Order Management System]
    E -->|Route to Symbol| F[Matching Engine]
    F -->|Generate Trades| G[Settlement Layer]
    G -->|Update Balances| H[(Database/Ledger)]
    F -->|Market Data| I[WebSocket Feed]
```

## Detailed engine structure
```mermaid
graph TD
    subgraph MarketManager
        OB1[OrderBook BTC-USD]
        OB2[OrderBook ETH-USD]
    end

    subgraph OrderBookInternal
        Direction{Side?}
        Direction -->|Bid| Bids[BTreeMap Price, Vec Order]
        Direction -->|Ask| Asks[BTreeMap Price, Vec Order]
        Seq[Internal arrival_seq Counter]
    end

    subgraph OrderStruct
        ID[OrderId]
        P[Price]
        Q[Quantity]
        S[arrival_seq]
    end

    OB1 -.-> Direction
    Bids --> OrderStruct
    Asks --> OrderStruct
```

## Matching mechanism
```mermaid
flowchart TD
    Start([Incoming OrderRequest]) --> Seq[Assign arrival_seq and create Order]
    Seq --> Loop{Quantity > 0?}
    
    Loop -- Yes --> BestPrice[Fetch Best Opposite Price from BTreeMap]
    BestPrice --> MatchCheck{Price Match?}
    
    MatchCheck -- Yes --> GetLevel[Fetch Price Level Vector]
    GetLevel --> GetMaker[Select first order - Time Priority]
    
    GetMaker --> Calc[Calculate qty_traded: min of Taker/Maker]
    Calc --> CreateTrade[Create Trade object with maker_arrival_seq]
    
    CreateTrade --> Update[Update quantities and Remove empty orders]
    Update --> Loop
    
    MatchCheck -- No --> AddToBook[Add remainder to BTreeMap]
    BestPrice -- No Opposite Orders --> AddToBook
    
    Loop -- No --> Finish([Return Trades Vector])
    AddToBook --> Finish
```

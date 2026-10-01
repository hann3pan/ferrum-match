# ferrum-match

Matching engine for a crypto exchange, in Rust.

Stack, lokale dev setup en featurelijst: zie README.md.

Leerproject: de owner leert Rust, komt van een C/C++- en TypeScript-achtergrond, en wil elke
wijziging kunnen volgen — niet alleen het resultaat.

## Workflow

- Create a GitHub issue for the task, work on a branch named `<issue-nr>-<short-name>`, and open
  a PR to main that references the issue.
- Conventional commits: `feat` / `fix` / `refactor` / `test` / `ci` / `docs`. Keep commits small
  and logical.
- Before finishing: `cargo fmt`, `cargo clippy --all-targets -- -D warnings` (fix warnings in
  files within the task's scope), and `cargo test` — all green.

## Commands

| | ferrum_match |
|---|---|
| Build | `cargo build` |
| Test (alle) | `cargo test` |
| Test (één) | `cargo test <naam-substring>` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Format check | `cargo fmt --check` |

Benchmarks en fuzz targets bestaan nog niet in deze repo.

## Always

- Represent price and quantity as fixed-point integers (e.g. ticks/lots as i64/u128, or
  `rust_decimal`) — never `f32`/`f64` for anything that represents money or an exchange rate.
  Floating point rounding is a correctness bug here, not a style nit. (`Price`/`Quantity` are
  already `u64` ticks/lots in `src/orderbook/types.rs` — keep it that way.)
- Keep the matching algorithm (price-time priority, order book mutation) as pure, synchronous,
  single-threaded logic with no I/O — network, persistence, and logging live in a separate layer
  that calls into it. This is the one place P&L-equivalent bugs happen; route all order-matching
  logic through it instead of recomputing matching rules elsewhere.
- Every order book mutation must be derived from an explicit, logged/persisted event (order
  accepted, matched, cancelled) before it's considered committed — the book's state must be
  reconstructable by replaying the event log from zero. Treat this the same as an audit trail.
- Validate every inbound order (symbol, side, price tick size, quantity lot size, account balance/
  margin) at the boundary before it touches the order book. Never let unvalidated client input
  reach matching logic.
- Write a property-based test (proptest) for every order book invariant you rely on: conservation
  of quantity (nothing created or destroyed by a match), price-time priority holding under
  arbitrary interleavings, no negative balances, no crossed book after matching completes.
  `tests/orderbook.rs` already has `proptest!` blocks — extend those rather than starting a
  parallel pattern.
- Derive account/user identity server-side from the authenticated session/API key on every order
  and balance query — never trust an account id supplied by the client payload.

## Ask first

- Before changing the matching algorithm itself (price-time priority rules, self-trade prevention,
  order types) — this is the financial correctness core, changes need explicit sign-off, not a
  drive-by refactor.
- Before changing the event log / WAL format or anything replay/recovery depends on — a silent
  format change breaks the ability to reconstruct past state.
- Before adding or upgrading a cryptography, networking, or exchange-connector dependency — check
  advisories first (`cargo audit`).
- Before changing anything that touches real exchange API keys/secrets or order-routing to a live
  venue.

## Never

- Never use `unwrap()`, `expect()`, or `panic!()` on a path that handles live order flow —
  a panic in the matching thread takes the book down. Propagate `Result` to a layer that can
  decide (reject the order, alert, degrade), matching this project's "errors propagate, no nested
  handling" rule below.
- Never use `unsafe` without a comment justifying the invariant it upholds and a test that would
  fail if that invariant breaks.
- Never let the matching engine block on I/O (network, disk, logging) inside the hot path — if it
  must happen, hand it to another thread/task and keep the book's critical section synchronous.
- Never log or persist API keys, private keys, or full account secrets in plaintext — same
  blast radius as leaking a service-role key.
- Never accept an order field's trust boundary assumption silently — if a quantity/price arrives
  as a string or float from an external feed, convert and validate explicitly at the edge, don't
  let it flow into the book un-normalized.

## Code standards

- Idiomatic, readable Rust over clever Rust — if a generic or trait trick saves 10 lines but is
  harder to read, don't use it.
- No `unwrap()` / `expect()` outside of tests — stricter than the live-order-flow rule under
  **Never** above (which explains why it matters most on the matching path); this rule applies
  to the rest of the codebase too.
- Domain code in `src/orderbook` does no I/O (see **Always** above): no `println!` — use
  `tracing` for anything that needs to be observed.
- Price-time priority behaviour must never change unless the task explicitly says so (see
  **Ask first** above).

## Implementation standards

Applies to every feature or fix, not just the items above. Full rationale:
`docs/standards/engineering-practices.md`.

- **Write the test first.** Red → green → refactor: a failing test that encodes the behavior,
  then the minimum code to pass it, then clean up.
- **Rule of three.** Don't extract a shared abstraction after the second similar block — wait for
  a genuine third occurrence.
- **DRY is about knowledge, not line-count.** A fact (a fee formula, a tick-size rule, a matching
  invariant) gets one source of truth from the first time it appears, regardless of how many
  copies of *code* currently reference it.
- **Keep indentation shallow — two levels deep at most.** Guard clause / early return / extract,
  instead of nesting further.
- **Three arguments per function, max.** Pass a struct past that.
- **Never pass a `bool` into a function.** Split into two named functions instead of branching
  on a flag inside.
- **Avoid a match/if-else chain that enumerates a fixed set of cases** that's expected to grow
  (order types, event types). Prefer a lookup table / trait object / enum dispatch so adding a
  case means adding one entry, not hunting down every call site.
- **`Result`, not panics, and no swallowing errors by matching and ignoring `Err`.** Propagate
  with `?` to a layer that can decide what to do — don't catch-and-ignore or catch-and-relog at
  every level.
- **Command-query separation.** A function that returns a value shouldn't also mutate the book;
  split "check and mutate" into a query and a command.
- **One concept per function/module.** If a module mixes matching logic with persistence or
  network concerns, split along that seam.
- **No speculative abstraction.** Don't add a generic parameter, trait, or config flag for a
  matching rule or order type that doesn't exist yet.

## Explaining (required)

Every PR description has a **"Rust notes"** section explaining ownership, borrowing and
lifetime decisions, any borrow-checker errors hit and how they were resolved, and tradeoffs
considered. Write it for a capable engineer who is new to Rust.

## Scope

Stay within the task. Anything else noticed goes under **"Follow-ups"** in the PR description,
not into the code.

## Working style

- Small, verifiable changes over large sweeping ones — tests and explicit verification carry
  more weight here than in a typical project, because correctness bugs are financial losses.
- Prefer reusing existing patterns in the codebase over introducing new ones for the same problem.

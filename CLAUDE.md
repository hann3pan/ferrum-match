# ferrum-match

Matching engine for a crypto exchange, in Rust.

Stack, local dev setup and feature list: see README.md.

Learning project: the owner is learning Rust, coming from a C/C++ and TypeScript background, and
wants to follow every change — not just the end result.

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
| Test (all) | `cargo test` |
| Test (one) | `cargo test <name-substring>` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Format check | `cargo fmt --check` |

Benchmarks and fuzz targets don't exist in this repo yet (benchmarks are tracked in
[#22](https://github.com/hann3pan/ferrum-match/issues/22); see README's Roadmap).

## Current rules

These are verified against the code as it exists today, not aspirational. If you change the code
in a way that breaks one of these, the rule — or the code — needs to change explicitly, not drift.

### Always

- Represent price and quantity as fixed-point integers — never `f32`/`f64` for anything that
  represents money or an exchange rate. `Price`/`Quantity` are already `u64` ticks/lots in
  `src/orderbook/types.rs`; keep it that way. Why: `docs/standards/engineering-practices.md`.
- Keep the matching algorithm (price-time priority, order book mutation in `src/orderbook`) pure,
  synchronous, single-threaded logic with no network or persistence I/O in the hot path. This is
  the one place P&L-equivalent bugs happen — route all order-matching logic through it instead of
  recomputing matching rules elsewhere. (`tracing` calls for observability are fine directly in
  this code — see Code standards below — it's network and persistence I/O that must live
  elsewhere.)
- Keep matching decisions deterministic: the same sequence of order requests must always produce
  the same trades and book state, with no dependence on wall-clock time, thread scheduling, or
  hash-map iteration order inside the matching path. (`Trade::timestamp` uses `SystemTime::now()`
  for display only — no matching decision reads it.) Why this matters beyond today: see the
  practices doc.
- Validate `price` and `quantity` at the boundary before an order reaches matching logic —
  `OrderBook::matching_order` rejects zero price/zero quantity, and the CLI
  (`src/modes/interactive.rs`) rejects non-numeric input before it ever becomes an `OrderRequest`.
- Write a property-based test (proptest) for every order book invariant you rely on.
  `tests/orderbook.rs` and `tests/invariants.rs` already cover quantity conservation, price-time
  priority under randomly interleaved submit/cancel operations, no crossed book, no empty price
  levels left behind, and a resting-order index that always matches the book's actual contents —
  extend those rather than starting a parallel pattern.

### Never

- Never use `unwrap()`, `expect()`, or `panic!()` on a path that handles live order flow — a panic
  in the matching thread takes the book down. Propagate `Result` to a layer that can decide
  (reject the order, alert, degrade).
- Never use `unsafe` without a comment justifying the invariant it upholds and a test that would
  fail if that invariant breaks.
- Never let the matching engine block on I/O (network, disk, logging) inside the hot path — if it
  must happen, hand it to another thread/task and keep the book's critical section synchronous.
- Never accept an order field's trust-boundary assumption silently — convert and validate
  explicitly at the edge rather than letting it flow into the book un-normalized. Today that means
  the CLI's string-to-`u64` parsing in `src/modes/interactive.rs`; the same principle applies to
  any future external input source.

### Ask first

- Before changing the matching algorithm itself (price-time priority rules) — this is the
  financial correctness core, changes need explicit sign-off, not a drive-by refactor.
- Before adding or upgrading a cryptography, networking, or exchange-connector dependency — check
  advisories first (`cargo audit`).

### Code standards

- Idiomatic, readable Rust over clever Rust — if a generic or trait trick saves 10 lines but is
  harder to read, don't use it.
- No `unwrap()` / `expect()` outside of tests — stricter than the live-order-flow rule under
  **Never** above (which explains why it matters most on the matching path); this rule applies
  to the rest of the codebase too.
- Domain code in `src/orderbook` does no I/O (see **Always** above): no `println!` — use
  `tracing` for anything that needs to be observed.
- Price-time priority behaviour must never change unless the task explicitly says so (see
  **Ask first** above).

## Target architecture rules

These describe where the project is heading, not what exists today — see README's "Current
status", "Roadmap" and "Target architecture" sections for the full picture. Don't write code as if
these already exist; when a task actually implements one of them, open an issue for it and move
the rule up into "Current rules" as part of that change.

### Always (target)

- Every order book mutation will be derived from an explicit, logged/persisted event (order
  accepted, matched, cancelled), and book state will be reconstructable by replaying the event log
  from zero. There is no event log or WAL yet — see README's "What doesn't exist yet" list.
- Validate symbol, price tick size, quantity lot size, and account balance/margin once those
  concepts exist. Today an `OrderRequest` only has `price`, `quantity` and `side`
  (`src/orderbook/types.rs`) — there is no symbol, tick size, lot size, or account/balance model
  at all yet.
- Derive account/user identity server-side from an authenticated session/API key on every order
  and balance query, once there is a session/API key to derive it from — there is no auth, session
  or gateway layer yet (see README's gateway-layer diagram and "Account balances, margin checks,
  or authentication" under "what doesn't exist yet").

### Never (target)

- Never log or persist API keys, private keys, or full account secrets in plaintext. This applies
  once the gateway/auth layer (see README's target architecture) exists; today there are no
  secrets anywhere in this codebase to leak.

### Ask first (target)

- Before adding self-trade prevention or a new order type (market orders, etc. — see README's
  Roadmap) — same financial-correctness-core sign-off bar as changing existing matching logic,
  since these are additions to the matching algorithm's rules, not unrelated features.
- Before changing the event log/WAL format once one is introduced, or anything replay/recovery
  depends on — a silent format change would break the ability to reconstruct past state. (No
  event log exists yet; this is a note for whoever builds it.)
- Before changing anything that touches real exchange API keys/secrets or order-routing to a live
  venue — no live venue connection exists yet (see README's gateway-layer target architecture).

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

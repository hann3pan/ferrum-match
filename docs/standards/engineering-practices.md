# Engineering practices

Full rationale behind the "Implementation standards" bullets in `CLAUDE.md`, plus the "why" behind
a few of its "Current rules" and "Target architecture rules". Read that file first — this doc
exists so the *why* doesn't have to be re-argued in every PR. Where a section below talks about
something CLAUDE.md lists under "Target architecture rules" (the event log, replay), it's
explaining the reasoning for a direction the project is heading, not describing code that exists
today — see README's "Current status" for what's actually implemented.

## Write the test first

Red → green → refactor: write a failing test that encodes the behavior you want, write the
minimum code to make it pass, then clean up with the test as a safety net. In a matching engine,
the cost of a bug is a wrong trade or a wrong balance, not a cosmetic glitch — writing the test
first forces you to state the invariant you're relying on (e.g. "quantity is conserved across a
match") before you write code that could violate it silently.

## Rule of three

Don't extract a shared abstraction after the second similar block — wait for a genuine third
occurrence. Two similar blocks might still diverge; a premature abstraction built on only two
data points tends to need its parameters reshaped (or its abstraction broken) once the third
case arrives, which costs more than the duplication did. Three real occurrences is enough
evidence that the shape is actually shared.

## DRY is about knowledge, not line-count

A *fact* — a fee formula, a tick-size rule, a matching invariant like "price-time priority" —
should have exactly one source of truth in the code, from the first time it appears, no matter
how many call sites reference it. This is different from deduplicating similar-looking code:
two functions can share no code and still violate DRY if they both hardcode the same fee
percentage, and two functions can look textually similar without violating DRY if they encode
unrelated facts that happen to coincide today. In a matching engine, letting a tick-size or
fee rule exist in two places is how they silently drift apart and produce a correctness bug
that only shows up in production reconciliation.

## Keep indentation shallow — two levels deep at most

Deep nesting (`if` inside `if` inside `match` inside a loop) hides the actual control flow and
makes it easy to miss a branch where an order should have been rejected. Use guard clauses,
early returns, or extract a function instead of nesting further. This is especially important
on the order-validation and matching paths, where every branch represents a business rule that
needs to be individually auditable.

## Three arguments per function, max

Beyond three positional arguments, call sites stop communicating what they mean (`place_order(1,
100, 5, true, false)` is unreadable) and it becomes easy to pass arguments in the wrong order —
arguments of the same type (two `u64`s, two `bool`s) are especially easy to transpose without
the compiler catching it. Pass a struct (`OrderRequest { side, price, quantity }`) past that
threshold so each field is named at the call site.

## Never pass a `bool` into a function

A `bool` parameter forces the reader to go look up what `true` means at this call site, and
invites the function to grow an internal `if flag { .. } else { .. }` branch that silently
becomes two different behaviors glued together. Split into two named functions instead —
`cancel_order` and `cancel_order_and_notify`, not `cancel_order(id, notify: bool)`. This matters
more than usual here because a flipped boolean on an order-handling path (e.g. `is_maker` vs
`is_taker`) is exactly the kind of bug that's invisible in a diff and expensive in production.

## Avoid a match/if-else chain that enumerates a fixed set of cases that's expected to grow

Order types and event types are the canonical example: a `match` with one arm per order type is
fine as code *today*, but every new order type then requires hunting down every `match` in the
codebase and adding a new arm, and it's easy to miss one (silently falling through to a default
case is worse than a compile error). Prefer a lookup table, a trait object (`dyn OrderTypeRule`),
or enum dispatch via a method so that adding a new order type means adding one new impl, and the
compiler (exhaustiveness checking on the enum, or a trait that must be implemented) tells you
if you forgot a spot.

## `Result`, not panics, and no swallowing errors

This project's Rust equivalent of "prefer exceptions over error codes": use `Result<T, E>` and
propagate with `?`, don't encode failure as a sentinel value (`-1`, `None` standing in for an
error, a bare `bool`) and don't use `panic!` as control flow. The anti-pattern to avoid is nested
`match` on nested `Result`s at every call site — matching an inner `Err`, logging it, and
returning `Ok(())` anyway. That's the same problem as nested try/catch in other languages: it
hides the actual error path, makes it look like the operation succeeded to the caller, and means
no single layer has enough context to decide what the right response actually is (reject the
order? retry? alert and degrade?). Propagate with `?` up to a layer that has that context, and
let *that* layer decide once.

This is why `CLAUDE.md`'s "Never" section bans `unwrap()`/`expect()`/`panic!()` on any path that
handles live order flow: a panic in the matching thread doesn't just fail one request, it can
take the whole book down, which is a much larger blast radius than a returned `Err`.

## Command-query separation

A function that returns a value shouldn't also mutate the book, and a function that mutates the
book shouldn't also decide and return whether it was valid to do so. Split "check and mutate"
into a query (`fn best_bid(&self) -> Option<Price>`) and a command (`fn apply_match(&mut self,
..)`). This matters in a matching engine because it will make the event-sourcing discipline under
CLAUDE.md's "Target architecture rules" easier to retrofit later: if mutation is already isolated
to a small set of command functions, wiring each one to emit an event is a local change, instead
of auditing every function that happens to also have a side effect.

## One concept per function/module

If a module mixes matching logic with persistence or network concerns, split along that seam.
This is the direct consequence of the "Always" rule that the matching algorithm must be "pure,
synchronous, single-threaded logic with no I/O": if persistence calls are interleaved into the
matching code, there's no single file you can read to verify the matching logic is correct, and
no way to unit-test matching behavior without a database or network in the loop.

## No speculative abstraction

Don't add a generic parameter, trait, or config flag for a matching rule or order type that
doesn't exist yet. An abstraction designed against a guessed future requirement almost never
matches the requirement once it actually arrives, and in the meantime it adds a layer of
indirection that every reader of the matching code has to pay for. Add the generalization when
the second (or per "rule of three", third) concrete case shows up, shaped by what that case
actually needs.

---

## Why floating point is banned for price and quantity

Most decimal fractions (0.1, 0.01, the kind of value a price or a fee percentage naturally takes)
have no exact representation in binary floating point — `f32`/`f64` store the nearest
representable approximation. A single comparison or print might look fine, but a matching engine
performs thousands of additions and subtractions across the lifetime of an order book (partial
fills, re-adds of remainder quantity, running balances), and each operation on an already-rounded
value compounds the error. Over enough matches, `f64` arithmetic can produce a balance that's off
by a fraction of a cent — which is either a reconciliation failure or, worse, an exploitable
rounding bias an attacker can repeatedly trigger. Representing price and quantity as integers
(ticks and lots, as `src/orderbook/types.rs` already does with `u64`) or as `rust_decimal` makes
every arithmetic operation exact, so "quantity is conserved across a match" is actually true
instead of true-to-within-epsilon.

## Why the matching engine must be deterministic

Given the same sequence of inputs (orders, cancels), the matching engine must always produce the
same sequence of outputs (trades, book state) — no reliance on wall-clock time, thread scheduling
order, hash-map iteration order, or any other source of nondeterminism inside the matching path
itself. This is already a "Current rule" in CLAUDE.md, not just a future concern — one thing
depends on it today, and two more will once the event log exists:

- **Reproducible bugs (today).** A matching bug that only reproduces 1 time in 20 because of
  iteration order or timing is far more expensive to find and fix than one that reproduces every
  time given the same input — which is also why the property-based tests in `tests/orderbook.rs`
  and `tests/invariants.rs` are run with many generated cases: they're only trustworthy as
  regression tests if a failing case is deterministically reproducible from its seed.
- **Crash recovery (target).** CLAUDE.md's target rule that book state must be reconstructable by
  replaying the event log from zero will only work if replaying the same events twice produces the
  same book both times. A nondeterministic matching step would make the replayed state diverge
  from what was actually committed, defeating the entire point of the event log. There is no event
  log yet (see README), so this doesn't apply today — but determinism has to be in place before
  one is added, not retrofitted after.
- **Audit (target).** Once replay exists, the only way to prove the engine did the right thing for
  a disputed trade is to replay the exact input sequence and get the exact same trade.
  Nondeterminism would turn every audit into "it probably did the right thing."

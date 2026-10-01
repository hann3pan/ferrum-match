//! Stateful property test: apply a random sequence of submit/cancel
//! operations to a fresh book and check every invariant the matching engine
//! relies on after *each* operation, not just at the end.

use ferrum_match::orderbook::types::{OrderBook, OrderId, Side};
use proptest::prelude::*;
use std::collections::HashSet;

/// Prices stay in a narrow band so buys and sells cross often.
const PRICE_RANGE: std::ops::RangeInclusive<u64> = 95..=105;
const QTY_RANGE: std::ops::RangeInclusive<u64> = 1..=50;

#[derive(Debug, Clone)]
enum CancelTarget {
    /// Resolved against the ids issued so far (`index % issued.len()`), so
    /// this always cancels something that was, at some point, a real order.
    Issued(usize),
    /// A raw id used as-is. Order ids are small and sequential, so this is
    /// usually not resting (and often not issued at all) but can coincide
    /// with a real one.
    Arbitrary(u64),
}

#[derive(Debug, Clone)]
enum Op {
    Submit { side: Side, price: u64, qty: u64 },
    Cancel(CancelTarget),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        5 => (prop_oneof![Just(Side::Bid), Just(Side::Ask)], PRICE_RANGE, QTY_RANGE)
            .prop_map(|(side, price, qty)| Op::Submit { side, price, qty }),
        2 => any::<usize>().prop_map(|i| Op::Cancel(CancelTarget::Issued(i))),
        1 => (0u64..250).prop_map(|id| Op::Cancel(CancelTarget::Arbitrary(id))),
    ]
}

fn ops_strategy() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op_strategy(), 1..200)
}

/// Ids the book actually reports as resting, read back through the public
/// accessors (not the index). Compared against `indexed_order_ids()` to
/// check the index doesn't drift from reality.
fn resting_ids_from_levels(book: &OrderBook) -> HashSet<OrderId> {
    let mut ids = HashSet::new();
    for side in [Side::Bid, Side::Ask] {
        for price in PRICE_RANGE {
            if let Some(level) = book.orders_at(side, price) {
                ids.extend(level.iter().map(|order| order.id));
            }
        }
    }
    ids
}

fn assert_no_empty_price_levels(book: &OrderBook) {
    for side in [Side::Bid, Side::Ask] {
        for price in PRICE_RANGE {
            if let Some(level) = book.orders_at(side, price) {
                assert!(
                    !level.is_empty(),
                    "empty price level left at {price:?}/{side:?}"
                );
            }
        }
    }
}

fn assert_arrival_seq_strictly_increasing(book: &OrderBook) {
    for side in [Side::Bid, Side::Ask] {
        for price in PRICE_RANGE {
            let Some(level) = book.orders_at(side, price) else {
                continue;
            };
            for pair in level.iter().collect::<Vec<_>>().windows(2) {
                assert!(
                    pair[0].arrival_seq < pair[1].arrival_seq,
                    "arrival_seq not increasing at {price:?}/{side:?}: {} then {}",
                    pair[0].arrival_seq,
                    pair[1].arrival_seq
                );
            }
        }
    }
}

proptest! {
    #[test]
    fn book_invariants_hold_after_every_op(ops in ops_strategy()) {
        let mut book = OrderBook::new();
        let mut issued_ids: Vec<u64> = Vec::new();

        let mut total_submitted: u64 = 0;
        let mut total_traded: u64 = 0;
        let mut total_cancelled: u64 = 0;

        for op in ops {
            match op {
                Op::Submit { side, price, qty } => {
                    let request = OrderBook::make_order_request(price, qty, side);
                    let trades = book
                        .matching_order(request)
                        .expect("price/qty are always non-zero in this test's domain");

                    // Every accepted submit consumes exactly one sequential
                    // id, so the nth accepted submit got id n+1.
                    issued_ids.push(issued_ids.len() as u64 + 1);

                    total_submitted += qty;
                    for trade in &trades {
                        total_traded += trade.quantity;

                        let within_taker_limit = match side {
                            Side::Bid => trade.price <= price,
                            Side::Ask => trade.price >= price,
                        };
                        prop_assert!(
                            within_taker_limit,
                            "trade price {} outside taker limit {} ({:?})",
                            trade.price, price, side
                        );
                    }
                }
                Op::Cancel(target) => {
                    let id = match target {
                        CancelTarget::Issued(i) if !issued_ids.is_empty() => {
                            issued_ids[i % issued_ids.len()]
                        }
                        CancelTarget::Issued(_) => 0,
                        CancelTarget::Arbitrary(id) => id,
                    };
                    if let Some(order) = book.cancel_order(OrderId(id)) {
                        total_cancelled += order.quantity;
                    }
                }
            }

            // -- invariants, checked after every single operation --

            if let (Some(bid), Some(ask)) = (book.best_bid(), book.best_ask()) {
                prop_assert!(bid < ask, "book crossed: best_bid {} >= best_ask {}", bid, ask);
            }

            prop_assert_eq!(
                book.indexed_order_ids(),
                resting_ids_from_levels(&book),
                "index does not match the book's actual resting orders"
            );

            assert_no_empty_price_levels(&book);
            assert_arrival_seq_strictly_increasing(&book);

            let resting_qty = book.total_quantity(Side::Bid) + book.total_quantity(Side::Ask);
            prop_assert_eq!(
                total_submitted,
                2 * total_traded + resting_qty + total_cancelled,
                "volume not conserved: submitted {}, traded {}, resting {}, cancelled {}",
                total_submitted, total_traded, resting_qty, total_cancelled
            );
        }
    }
}

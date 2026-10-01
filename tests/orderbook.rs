use ferrum_match::orderbook::types::{OrderBook, OrderError, OrderId, Side};
use proptest::prelude::*;

fn empty_book() -> OrderBook {
    OrderBook::new()
}

/// Seeds the book with a resting order via the matching path. The caller is
/// responsible for choosing a price that does not cross the opposite side.
fn rest(book: &mut OrderBook, price: u64, qty: u64, side: Side) {
    book.matching_order(OrderBook::make_order_request(price, qty, side))
        .expect("seeding a non-crossing order must not be rejected");
}

#[cfg(test)]
mod orderbook {
    use super::*;

    #[test]
    fn should_match_exact_when_buy_order() {
        let mut book = empty_book();
        rest(&mut book, 100, 10, Side::Ask);

        let order2 = OrderBook::make_order_request(100, 10, Side::Bid);
        let trades = book.matching_order(order2).unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].taker_order_id, OrderId(2));
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].price, 100);
        assert_eq!(trades[0].quantity, 10);
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_match_exact_when_sell_order() {
        let mut book = empty_book();
        rest(&mut book, 150, 25, Side::Bid);

        let trades = book
            .matching_order(OrderBook::make_order_request(150, 25, Side::Ask))
            .unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].taker_order_id, OrderId(2));
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].price, 150);
        assert_eq!(trades[0].quantity, 25);
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_partial_fill_when_incoming_buy_order_is_larger() {
        let mut book = empty_book();
        rest(&mut book, 100, 5, Side::Ask);

        let trades = book
            .matching_order(OrderBook::make_order_request(100, 20, Side::Bid))
            .unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].taker_order_id, OrderId(2));
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].price, 100);
        assert_eq!(trades[0].quantity, 5);
        let best_bid = book.orders_at(Side::Bid, 100).unwrap();
        assert_eq!(best_bid[0].quantity, 15);
        assert_eq!(best_bid[0].price, 100);
        assert_eq!(best_bid[0].id, OrderId(2));
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_partial_fill_when_incoming_sell_order_is_larger() {
        let mut book = empty_book();
        rest(&mut book, 150, 12, Side::Bid);

        let trades = book
            .matching_order(OrderBook::make_order_request(150, 23, Side::Ask))
            .unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].taker_order_id, OrderId(2));
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].price, 150);
        assert_eq!(trades[0].quantity, 12);
        let best_ask = book.orders_at(Side::Ask, 150).unwrap();
        assert_eq!(best_ask[0].quantity, 11);
        assert_eq!(best_ask[0].price, 150);
        assert_eq!(best_ask[0].id, OrderId(2));
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn should_partial_fill_when_book_sell_order_is_larger() {
        let mut book = empty_book();
        rest(&mut book, 180, 25, Side::Ask);

        let trades = book
            .matching_order(OrderBook::make_order_request(180, 10, Side::Bid))
            .unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].taker_order_id, OrderId(2));
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].quantity, 10);
        assert_eq!(trades[0].price, 180);
        let best_ask = book.orders_at(Side::Ask, 180).unwrap();
        assert_eq!(best_ask[0].quantity, 15);
        assert_eq!(best_ask[0].price, 180);
        assert_eq!(best_ask[0].id, OrderId(1));
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn should_partial_fill_when_book_buy_order_is_larger() {
        let mut book = empty_book();
        rest(&mut book, 90, 35, Side::Bid);

        let trades = book
            .matching_order(OrderBook::make_order_request(90, 8, Side::Ask))
            .unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].taker_order_id, OrderId(2));
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].quantity, 8);
        assert_eq!(trades[0].price, 90);
        let best_bid = book.orders_at(Side::Bid, 90).unwrap();
        assert_eq!(best_bid[0].quantity, 27);
        assert_eq!(best_bid[0].price, 90);
        assert_eq!(best_bid[0].id, OrderId(1));
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_not_cross_asks_when_buy_order() {
        let mut book = empty_book();
        rest(&mut book, 105, 10, Side::Ask);
        rest(&mut book, 110, 10, Side::Ask);

        let trades = book
            .matching_order(OrderBook::make_order_request(100, 10, Side::Bid))
            .unwrap();
        assert!(trades.is_empty());
        let bids = book.orders_at(Side::Bid, 100).unwrap();
        assert_eq!(bids[0].id, OrderId(3));
        assert_eq!(book.price_level_count(Side::Bid), 1);
        assert_eq!(book.price_level_count(Side::Ask), 2);
    }

    #[test]
    fn should_not_cross_bids_when_sell_order() {
        let mut book = empty_book();
        rest(&mut book, 95, 10, Side::Bid);
        rest(&mut book, 90, 10, Side::Bid);

        let trades = book
            .matching_order(OrderBook::make_order_request(100, 10, Side::Ask))
            .unwrap();
        assert!(trades.is_empty());
        let asks = book.orders_at(Side::Ask, 100).unwrap();
        assert_eq!(asks[0].id, OrderId(3));
        assert_eq!(book.price_level_count(Side::Ask), 1);
        assert_eq!(book.price_level_count(Side::Bid), 2);
    }

    #[test]
    fn should_match_fifo_within_price_level_when_buy_order() {
        let mut book = empty_book();
        rest(&mut book, 100, 5, Side::Ask);
        rest(&mut book, 100, 5, Side::Ask);
        rest(&mut book, 100, 5, Side::Ask);

        let trades = book
            .matching_order(OrderBook::make_order_request(100, 12, Side::Bid))
            .unwrap();

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].maker_arrival_seq, 0);
        assert_eq!(trades[0].taker_order_id, OrderId(4));
        assert_eq!(trades[0].quantity, 5);
        assert_eq!(trades[1].maker_order_id, OrderId(2));
        assert_eq!(trades[1].maker_arrival_seq, 1);
        assert_eq!(trades[1].taker_order_id, OrderId(4));
        assert_eq!(trades[1].quantity, 5);
        assert_eq!(trades[2].maker_order_id, OrderId(3));
        assert_eq!(trades[2].maker_arrival_seq, 2);
        assert_eq!(trades[2].taker_order_id, OrderId(4));
        assert_eq!(trades[2].quantity, 2);
        let best_ask = book.orders_at(Side::Ask, 100).unwrap();
        assert_eq!(best_ask[0].id, OrderId(3));
        assert_eq!(best_ask[0].quantity, 3);
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn should_match_fifo_within_price_level_when_sell_order() {
        let mut book = empty_book();
        rest(&mut book, 70, 3, Side::Bid);
        rest(&mut book, 70, 4, Side::Bid);

        let trades = book
            .matching_order(OrderBook::make_order_request(70, 6, Side::Ask))
            .unwrap();

        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].maker_arrival_seq, 0);
        assert_eq!(trades[0].taker_order_id, OrderId(3));
        assert_eq!(trades[0].quantity, 3);
        assert_eq!(trades[1].maker_order_id, OrderId(2));
        assert_eq!(trades[1].maker_arrival_seq, 1);
        assert_eq!(trades[1].taker_order_id, OrderId(3));
        assert_eq!(trades[1].quantity, 3);
        let best_bid = book.orders_at(Side::Bid, 70).unwrap();
        assert_eq!(best_bid[0].id, OrderId(2));
        assert_eq!(best_bid[0].quantity, 1);
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_match_multi_price_levels_when_buy_order() {
        let mut book = empty_book();
        rest(&mut book, 100, 5, Side::Ask);
        rest(&mut book, 102, 6, Side::Ask);
        rest(&mut book, 105, 7, Side::Ask);

        let trades = book
            .matching_order(OrderBook::make_order_request(105, 12, Side::Bid))
            .unwrap();

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].price, 100);
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].taker_order_id, OrderId(4));
        assert_eq!(trades[1].price, 102);
        assert_eq!(trades[1].maker_order_id, OrderId(2));
        assert_eq!(trades[1].taker_order_id, OrderId(4));
        assert_eq!(trades[2].price, 105);
        assert_eq!(trades[2].maker_order_id, OrderId(3));
        assert_eq!(trades[2].taker_order_id, OrderId(4));
        let remaining = book.orders_at(Side::Ask, 105).unwrap();
        assert_eq!(remaining[0].quantity, 6);
        assert!(book.best_bid().is_none());
        assert_eq!(book.price_level_count(Side::Ask), 1);
    }

    #[test]
    fn should_match_multi_price_levels_when_sell_order() {
        let mut book = empty_book();
        rest(&mut book, 100, 5, Side::Bid);
        rest(&mut book, 102, 6, Side::Bid);
        rest(&mut book, 105, 7, Side::Bid);

        let trades = book
            .matching_order(OrderBook::make_order_request(95, 12, Side::Ask))
            .unwrap();

        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].price, 105);
        assert_eq!(trades[0].maker_order_id, OrderId(3));
        assert_eq!(trades[0].taker_order_id, OrderId(4));
        assert_eq!(trades[1].price, 102);
        assert_eq!(trades[1].maker_order_id, OrderId(2));
        assert_eq!(trades[1].taker_order_id, OrderId(4));
        let remaining_102 = book.orders_at(Side::Bid, 102).unwrap();
        assert_eq!(remaining_102[0].quantity, 1);
        let remaining_100 = book.orders_at(Side::Bid, 100).unwrap();
        assert_eq!(remaining_100[0].quantity, 5);
        assert_eq!(book.price_level_count(Side::Bid), 2);
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_cross_partially_due_to_price_limit_when_buy_order() {
        let mut book = empty_book();
        rest(&mut book, 100, 5, Side::Ask);
        rest(&mut book, 101, 5, Side::Ask);
        rest(&mut book, 102, 5, Side::Ask);

        let trades = book
            .matching_order(OrderBook::make_order_request(101, 20, Side::Bid))
            .unwrap();

        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].price, 100);
        assert_eq!(trades[0].quantity, 5);
        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].taker_order_id, OrderId(4));
        assert_eq!(trades[1].price, 101);
        assert_eq!(trades[1].quantity, 5);
        assert_eq!(trades[1].maker_order_id, OrderId(2));
        assert_eq!(trades[1].taker_order_id, OrderId(4));
        let remaining = book.orders_at(Side::Bid, 101).unwrap();
        assert_eq!(remaining[0].quantity, 10);
        assert_eq!(book.price_level_count(Side::Ask), 1);
    }

    #[test]
    fn should_cross_partially_due_to_price_limit_when_sell_order() {
        let mut book = empty_book();
        rest(&mut book, 100, 5, Side::Bid);
        rest(&mut book, 101, 5, Side::Bid);
        rest(&mut book, 102, 5, Side::Bid);

        let trades = book
            .matching_order(OrderBook::make_order_request(101, 30, Side::Ask))
            .unwrap();

        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].price, 102);
        assert_eq!(trades[0].quantity, 5);
        assert_eq!(trades[0].maker_order_id, OrderId(3));
        assert_eq!(trades[0].taker_order_id, OrderId(4));
        assert_eq!(trades[1].price, 101);
        assert_eq!(trades[1].quantity, 5);
        assert_eq!(trades[1].maker_order_id, OrderId(2));
        assert_eq!(trades[1].taker_order_id, OrderId(4));
        let remaining = book.orders_at(Side::Ask, 101).unwrap();
        assert_eq!(remaining[0].quantity, 20);
        assert_eq!(book.price_level_count(Side::Bid), 1);
    }

    #[test]
    fn should_process_trade_as_buy_order_when_book_is_empty() {
        let mut book = empty_book();

        let trades = book
            .matching_order(OrderBook::make_order_request(100, 3, Side::Bid))
            .unwrap();

        assert!(trades.is_empty());
        let level = book.orders_at(Side::Bid, 100).unwrap();
        assert_eq!(level[0].quantity, 3);
        assert_eq!(level[0].price, 100);
        assert_eq!(level[0].id, OrderId(1));
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_process_trade_as_sell_order_when_book_is_empty() {
        let mut book = empty_book();

        let trades = book
            .matching_order(OrderBook::make_order_request(99, 6, Side::Ask))
            .unwrap();

        assert!(trades.is_empty());
        let level = book.orders_at(Side::Ask, 99).unwrap();
        assert_eq!(level[0].quantity, 6);
        assert_eq!(level[0].price, 99);
        assert_eq!(level[0].id, OrderId(1));
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn should_cleanup_price_level_when_buy_order() {
        let mut book = empty_book();
        rest(&mut book, 50, 2, Side::Ask);

        book.matching_order(OrderBook::make_order_request(50, 2, Side::Bid))
            .unwrap();

        assert!(book.orders_at(Side::Ask, 50).is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_cleanup_price_level_when_sell_order() {
        let mut book = empty_book();
        rest(&mut book, 70, 6, Side::Bid);

        book.matching_order(OrderBook::make_order_request(70, 6, Side::Ask))
            .unwrap();

        assert!(book.orders_at(Side::Bid, 70).is_none());
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn should_reject_zero_quantity_when_buy_order() {
        let mut book = empty_book();

        let result = book.matching_order(OrderBook::make_order_request(70, 0, Side::Bid));

        assert!(
            matches!(result, Err(OrderError::ZeroQuantity)),
            "{result:?}"
        );
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_reject_zero_quantity_when_sell_order() {
        let mut book = empty_book();

        let result = book.matching_order(OrderBook::make_order_request(70, 0, Side::Ask));

        assert!(
            matches!(result, Err(OrderError::ZeroQuantity)),
            "{result:?}"
        );
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_reject_zero_price_when_buy_order() {
        let mut book = empty_book();

        let result = book.matching_order(OrderBook::make_order_request(0, 5, Side::Bid));

        assert!(matches!(result, Err(OrderError::ZeroPrice)), "{result:?}");
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn should_reject_zero_price_when_sell_order() {
        let mut book = empty_book();

        let result = book.matching_order(OrderBook::make_order_request(0, 5, Side::Ask));

        assert!(matches!(result, Err(OrderError::ZeroPrice)), "{result:?}");
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn rejected_order_does_not_consume_an_id() {
        let mut book = empty_book();

        let rejected = book.matching_order(OrderBook::make_order_request(0, 5, Side::Bid));
        assert!(rejected.is_err());

        let rejected_again = book.matching_order(OrderBook::make_order_request(10, 0, Side::Bid));
        assert!(rejected_again.is_err());

        let accepted = book.matching_order(OrderBook::make_order_request(10, 5, Side::Bid));
        assert!(accepted.is_ok());

        let level = book.orders_at(Side::Bid, 10).unwrap();
        assert_eq!(level[0].id, OrderId(1));
    }

    #[test]
    fn fully_filled_maker_is_removed_from_index() {
        let mut book = empty_book();
        rest(&mut book, 100, 10, Side::Ask);

        book.matching_order(OrderBook::make_order_request(100, 10, Side::Bid))
            .unwrap();

        assert!(book.cancel_order(OrderId(1)).is_none());
    }

    #[test]
    fn fully_filled_taker_is_not_indexed() {
        let mut book = empty_book();
        rest(&mut book, 100, 10, Side::Ask);

        book.matching_order(OrderBook::make_order_request(100, 10, Side::Bid))
            .unwrap();

        assert!(book.cancel_order(OrderId(2)).is_none());
    }

    #[test]
    fn cancel_partially_filled_resting_order_returns_remaining_qty() {
        let mut book = empty_book();
        rest(&mut book, 100, 10, Side::Ask);

        book.matching_order(OrderBook::make_order_request(100, 4, Side::Bid))
            .unwrap();

        let remaining = book
            .cancel_order(OrderId(1))
            .expect("partially filled order should still be resting");
        assert_eq!(remaining.quantity, 6);
        assert!(book.orders_at(Side::Ask, 100).is_none());
    }

    #[test]
    fn should_handle_orders_based_on_arrival_sequence() {
        let mut book = empty_book();
        rest(&mut book, 5, 10, Side::Bid);
        rest(&mut book, 5, 10, Side::Bid);

        let trades = book
            .matching_order(OrderBook::make_order_request(5, 10, Side::Ask))
            .unwrap();

        assert_eq!(trades[0].maker_order_id, OrderId(1));
        assert_eq!(trades[0].maker_arrival_seq, 0);
        assert!(book.best_ask().is_none());
        assert_eq!(book.price_level_count(Side::Bid), 1);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    #[test]
    fn test_volume_conservation(
        initial_qty in 1u64..1000u64,
        incoming_qty in 1u64..1000u64,
        price in 1u64..1000u64
    ) {
        let mut book = empty_book();

        rest(&mut book, price, initial_qty, Side::Ask);

        let taker_req = OrderBook::make_order_request(price, incoming_qty, Side::Bid);
        let trades = book.matching_order(taker_req).unwrap();

        let traded_volume: u64 = trades.iter().map(|t| t.quantity).sum();

        let remaining_ask_volume = book.total_quantity(Side::Ask);
        let remaining_bid_volume = book.total_quantity(Side::Bid);

        let total_after = traded_volume * 2 + remaining_ask_volume + remaining_bid_volume;
        let total_before = initial_qty + incoming_qty;

        prop_assert_eq!(total_after, total_before,
            "Lost volume! Before: {}, After: {}", total_before, total_after);
    }
}

proptest! {
        #[test]
        fn test_time_priority_is_respected(
            maker_qty_list in prop::collection::vec(1u64..100u64, 2..10)
        ) {
            let mut book = empty_book();
            let price = 100;

            for qty in maker_qty_list.iter() {
                rest(&mut book, price, *qty, Side::Ask);
            }

            let total_taker_qty: u64 = maker_qty_list.iter().sum();
            let taker_req = OrderBook::make_order_request(price, total_taker_qty, Side::Bid);
            let trades = book.matching_order(taker_req).unwrap();

            for i in 0..(trades.len() - 1) {
                prop_assert!(
                    trades[i].maker_arrival_seq < trades[i+1].maker_arrival_seq,
                    "Time priority violated! Trade {} (seq {}) came after Trade {} (seq {})",
                    i+1, trades[i+1].maker_arrival_seq, i, trades[i].maker_arrival_seq
                );
            }
        }
}

use tracing::{debug, info, instrument, trace};

use super::types::{
    Order, OrderBook, OrderError, OrderId, OrderRequest, Price, Quantity, Side, Trade, Trades,
};

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::SystemTime;

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            next_seq: 0,
            order_id_counter: 1,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_index: HashMap::new(),
        }
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.best_price(Side::Bid)
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.best_price(Side::Ask)
    }

    /// Resting orders at `price` on `side`, FIFO (oldest first).
    pub fn orders_at(&self, side: Side, price: Price) -> Option<&VecDeque<Order>> {
        self.side_map(side).get(&price)
    }

    /// Resting price levels on `side`, best price first: bids descending
    /// from the highest price, asks ascending from the lowest. Each level
    /// is `(price, orders)` with `orders` in FIFO arrival order, exactly as
    /// stored — the returned iterator borrows from `self` and cannot outlive
    /// it.
    pub fn levels(&self, side: Side) -> impl Iterator<Item = (Price, &VecDeque<Order>)> {
        let ascending = self
            .side_map(side)
            .iter()
            .map(|(&price, orders)| (price, orders));

        match side {
            Side::Bid => {
                Box::new(ascending.rev()) as Box<dyn Iterator<Item = (Price, &VecDeque<Order>)>>
            }
            Side::Ask => Box::new(ascending),
        }
    }

    /// Number of distinct price levels currently resting on `side`.
    pub fn price_level_count(&self, side: Side) -> usize {
        self.side_map(side).len()
    }

    /// Total number of resting orders across both sides.
    pub fn resting_order_count(&self) -> usize {
        self.order_index.len()
    }

    /// Ids the index currently tracks as resting. Lets tests cross-check the
    /// index against the book's actual contents (bids/asks are private);
    /// production code has no use for the raw id set, so this only exists
    /// under the `test-utils` feature.
    #[cfg(feature = "test-utils")]
    pub fn indexed_order_ids(&self) -> std::collections::HashSet<OrderId> {
        self.order_index.keys().copied().collect()
    }

    /// Sum of resting quantity across all price levels on `side`.
    pub fn total_quantity(&self, side: Side) -> Quantity {
        self.side_map(side)
            .values()
            .flatten()
            .map(|order| order.quantity)
            .sum()
    }

    fn best_price(&self, side: Side) -> Option<Price> {
        match side {
            Side::Bid => self.bids.keys().next_back().copied(),
            Side::Ask => self.asks.keys().next().copied(),
        }
    }

    fn side_map(&self, side: Side) -> &BTreeMap<Price, VecDeque<Order>> {
        match side {
            Side::Bid => &self.bids,
            Side::Ask => &self.asks,
        }
    }

    fn side_map_mut(&mut self, side: Side) -> &mut BTreeMap<Price, VecDeque<Order>> {
        match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        }
    }

    fn next_order_id(&mut self) -> OrderId {
        let id = OrderId(self.order_id_counter);
        self.order_id_counter += 1;
        trace!(new_id = id.0, "Generated next order ID");
        id
    }

    pub fn make_order_request(price: Price, quantity: Quantity, side: Side) -> OrderRequest {
        OrderRequest {
            price,
            quantity,
            side,
        }
    }

    #[instrument(skip(self), fields(order_id))]
    fn make_order(&mut self, price: Price, quantity: Quantity, side: Side) -> Order {
        let id = self.next_order_id();
        tracing::Span::current().record("order_id", id.0);

        let order = Order {
            id,
            price,
            quantity,
            arrival_seq: self.next_seq,
        };

        self.next_seq += 1;
        debug!(id = %order.id.0, price = %order.price, qty = %order.quantity, "Order struct initialized");
        order
    }

    /// Cancels a resting order. Returns `None` if the id is unknown or
    /// already fully filled/cancelled — the index only ever tracks resting orders.
    pub fn cancel_order(&mut self, order_id: OrderId) -> Option<Order> {
        let (side, price) = *self.order_index.get(&order_id)?;
        let book_side = self.side_map_mut(side);
        let level = book_side.get_mut(&price)?;
        let order = Self::remove_from_level(level, order_id)?;

        if level.is_empty() {
            book_side.remove(&price);
        }

        self.order_index.remove(&order_id);
        Some(order)
    }

    fn remove_from_level(level: &mut VecDeque<Order>, order_id: OrderId) -> Option<Order> {
        let pos = level.iter().position(|o| o.id == order_id)?;
        level.remove(pos)
    }

    /// The single place an order starts resting: pushes it onto the book
    /// *and* records it in `order_index` in the same step, so the index can
    /// never drift from "the set of orders currently resting on a level".
    fn add_order_internal(&mut self, order: Order, side: Side) {
        trace!(id = %order.id.0, side = ?side, "Inserting order into book");
        self.order_index.insert(order.id, (side, order.price));
        self.side_map_mut(side)
            .entry(order.price)
            .or_default()
            .push_back(order);
    }

    fn make_trade(
        taker_order_id: OrderId,
        maker_order_id: OrderId,
        maker_arrival_seq: u64,
        price: Price,
        quantity: Quantity,
        timestamp: SystemTime,
    ) -> Trade {
        let trade = Trade {
            taker_order_id,
            maker_order_id,
            maker_arrival_seq,
            price,
            quantity,
            timestamp,
        };

        info!(
            target = "trades",
            taker = %trade.taker_order_id.0,
            maker = %trade.maker_order_id.0,
            price = %trade.price,
            qty = %trade.quantity,
            "Execution occurred"
        );

        trade
    }

    /// Whether a resting order at `resting_price` would trade against a taker
    /// of `taker_side` limited at `taker_price`.
    fn crosses(taker_side: Side, taker_price: Price, resting_price: Price) -> bool {
        match taker_side {
            Side::Bid => resting_price <= taker_price,
            Side::Ask => resting_price >= taker_price,
        }
    }

    #[instrument(skip(self, incoming), fields(incoming_id = %incoming.id.0, side = ?side))]
    fn match_order(&mut self, mut incoming: Order, side: Side) -> Trades {
        let mut trades = Trades::default();
        let opposite = side.opposite();

        debug!(
            qty = incoming.quantity,
            price = incoming.price,
            "Start matching order"
        );

        while incoming.quantity > 0 {
            let Some(resting_price) = self.best_price(opposite) else {
                trace!("No resting orders available on the opposite side");
                break;
            };

            if !Self::crosses(side, incoming.price, resting_price) {
                trace!(resting_price = %resting_price, incoming_price = %incoming.price, "Price gap reached; stopping match");
                break;
            }

            // Borrow the field directly (not through `side_map_mut`, which takes
            // `&mut self`) so this mutable borrow of `self.bids`/`self.asks` stays
            // disjoint from `self.order_index`: we still need to mutate the index
            // below while `level`/`maker` (derived from this borrow) are alive.
            let opposite_book = match opposite {
                Side::Bid => &mut self.bids,
                Side::Ask => &mut self.asks,
            };
            let Some(level) = opposite_book.get_mut(&resting_price) else {
                break;
            };
            let Some(maker) = level.front_mut() else {
                break;
            };

            let qty_traded = maker.quantity.min(incoming.quantity);
            let maker_id = maker.id;
            let maker_arrival_seq = maker.arrival_seq;

            maker.quantity -= qty_traded;
            incoming.quantity -= qty_traded;
            let maker_fully_filled = maker.quantity == 0;

            trace!(maker_id = %maker_id.0, qty = %qty_traded, "Matching against price level");

            trades.push(Self::make_trade(
                incoming.id,
                maker_id,
                maker_arrival_seq,
                resting_price,
                qty_traded,
                SystemTime::now(),
            ));

            if maker_fully_filled {
                level.pop_front();
                trace!(maker_id = %maker_id.0, "Maker order fully filled, removing from level");
            }
            if level.is_empty() {
                opposite_book.remove(&resting_price);
                trace!(price = %resting_price, "Price level empty, removing from book");
            }

            if maker_fully_filled {
                self.order_index.remove(&maker_id);
            }
        }

        // A fully filled taker never rested, so it was never added to
        // `order_index` in the first place (see `add_order_internal`) —
        // nothing to remove here. Only a remainder that starts resting needs
        // to enter the index, which `add_order_internal` does on its own.
        if incoming.quantity > 0 {
            debug!(remaining_qty = %incoming.quantity, "Order not fully filled, adding remainder to book");
            self.add_order_internal(incoming, side);
        }

        trades
    }

    /// Entry point for all order flow. Validates the request, then matches it
    /// against the book. Rejects zero price/quantity before an order id or
    /// arrival sequence number is consumed.
    #[instrument(skip(self), fields(side = ?incoming.side, p = %incoming.price, q = %incoming.quantity))]
    pub fn matching_order(&mut self, incoming: OrderRequest) -> Result<Trades, OrderError> {
        if incoming.price == 0 {
            return Err(OrderError::ZeroPrice);
        }
        if incoming.quantity == 0 {
            return Err(OrderError::ZeroQuantity);
        }

        info!("Processing incoming order request");
        let order = self.make_order(incoming.price, incoming.quantity, incoming.side);
        Ok(self.match_order(order, incoming.side))
    }
}

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::SystemTime;

pub type Price = u64;
pub type Quantity = u64;
pub type ArrivalSeq = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

impl Side {
    pub fn opposite(self) -> Side {
        match self {
            Side::Bid => Side::Ask,
            Side::Ask => Side::Bid,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: OrderId,
    pub price: Price,
    pub quantity: Quantity,
    pub arrival_seq: ArrivalSeq,
}

#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub side: Side,
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Debug, Clone)]
pub struct Trade {
    pub taker_order_id: OrderId,
    pub maker_order_id: OrderId,
    pub maker_arrival_seq: u64,
    pub price: Price,
    pub quantity: Quantity,
    pub timestamp: SystemTime,
}

pub type Trades = Vec<Trade>;

/// Rejection reasons for order submission. Validated before an order id or
/// arrival sequence number is consumed, so a rejected order never burns either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderError {
    ZeroPrice,
    ZeroQuantity,
}

pub struct OrderBook {
    pub(super) next_seq: u64,
    pub(super) order_id_counter: u64,
    pub(super) bids: BTreeMap<Price, VecDeque<Order>>,
    pub(super) asks: BTreeMap<Price, VecDeque<Order>>,
    pub(super) order_index: HashMap<OrderId, (Side, Price)>,
}

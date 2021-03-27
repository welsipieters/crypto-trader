pub mod balances;
pub mod coin;
pub mod orderbook;
pub mod orderbook_old;
pub mod treasury;

use crate::crypto::orderbook::OrderSide;
use crate::exchanges::mandala::utils::OrderType;

pub struct Fees {
    taker: f64,
    maker: f64,
}

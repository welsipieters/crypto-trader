// extern crate uuid;
//
// use std::collections::VecDeque;
// use self::uuid::Uuid;
// use std::ops::RangeInclusive;
// use anyhow::Result;
// use std::fmt::{Display, Formatter};
// use serde::{Deserialize, Serialize};
// use serde_repr::{Serialize_repr, Deserialize_repr};
//
// const MAX_ORDERBOOK_SIZE: usize = 20000 * 100;
//
// #[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialOrd, PartialEq)]
// pub enum Side {
//     Buy,
//     Sell
// }
//
//
// #[derive(Debug, Serialize_repr, Deserialize_repr, Copy, Clone, PartialOrd, PartialEq)]
// #[repr(u8)]
// pub enum OrderSide {
//     Buy = 0,
//     Sell = 1
// }
//
// #[derive(Debug,Serialize_repr, Deserialize_repr, Copy, Clone, PartialOrd, PartialEq)]
// #[repr(u8)]
// pub enum OrderType {
//     Limit = 1,
//     Market = 2,
//     StopLoss = 3,
//     StopLossLimit = 4,
//     TakeProfit = 5,
//     TakeProfitLimit = 6,
//     LimitMake = 7
// }
//
// #[derive(Debug, Serialize_repr, Deserialize_repr, Copy, Clone, PartialOrd, PartialEq)]
// #[repr(u8)]
// pub enum OrderStatus {
//     New = 0,
//     PartiallyFilled = 1,
//     Filled = 2,
//     Canceled = 3,
//     PendingCancel = 4,
//     Rejected = 5,
//     Expired = 6
// }
//
//
// pub struct OrderRequest {
//     pub symbol: String,
//     pub side: Side,
//     pub order_type: OrderType,
//     pub quantity: Option<f64>,
//     pub market_order_quantity: Option<f64>,
//     pub price: Option<f64>,
//     pub stop_price: Option<f64>,
// }
//
// #[derive(Debug)]
// pub struct Record {
//     pub price: f64,
//     pub size: f64,
//     pub id: uuid::Uuid
// }
//
// impl Record {
//     pub fn new (id: Uuid, price: f64, size: f64) -> Self {
//         Self {
//             price,
//             size,
//             id
//         }
//     }
// }
//
// #[derive(Debug)]
// pub struct OrderBook {
//     pair: String,
//     book: Vec<VecDeque<(f64, Uuid)>>,
//     bid: usize,
//     ask: usize,
//     last_match: usize,
// }
//
// impl OrderBook {
//     pub fn new<T: Into<String>>(pair: T) -> Self {
//         Self {
//             pair: pair.into(),
//             book: vec![VecDeque::new(); MAX_ORDERBOOK_SIZE],
//             bid: std::usize::MIN,
//             ask: std::usize::MAX,
//             last_match: 0
//         }
//     }
//
//     pub fn get_bid(&self) -> Option<f64> {
//         if self.bid == std::usize::MIN {
//             return None;
//         }
//
//         Some(self.bid as f64 / 1000.0)
//     }
//
//     pub fn get_ask(&self) -> Option<f64> {
//         if self.ask == std::usize::MAX {
//             return None;
//         }
//
//         Some(self.ask as f64 / 1000.0)
//     }
//
//     pub fn get_last_match(&self) -> Option<f64> {
//         if self.last_match == 0 {
//             return None;
//         }
//
//         Some(self.last_match as f64 / 1000.0)
//     }
//
//     fn side(&self, range: RangeInclusive<usize>) -> Vec<f64> {
//         self.book[range]
//             .iter()
//             .map(|x| {
//                 x.iter().map(|y| y.0).sum()
//             })
//             .collect::<Vec<_>>()
//     }
//
//     pub fn bids(&self, size: usize) -> Vec<f64> {
//         self.side((self.bid + 1 - size) ..= self.bid)
//     }
//
//     pub fn asks(&self, size: usize) -> Vec<f64> {
//         self.side(self.ask ..= (self.ask + size - 1))
//     }
//
//     fn get_price_id(&self, price: f64) -> Result<usize> {
//         let price_id = (price * 1000.0).round() as usize;
//
//         Ok(price_id)
//     }
//
//     pub fn open(&mut self, side: Side, record: Record) -> Result<()> {
//         let price_id = self.get_price_id(record.price)?;
//
//         match side {
//             Side::Buy => {
//                 if price_id > self.bid {
//                     self.bid = price_id
//                 }
//             }
//             Side::Sell => {
//                 if price_id < self.ask {
//                     self.ask = price_id
//                 }
//             }
//         }
//
//         // assert!(self.bid <= self.ask, "bid exceeds ask ({} >= {}) on record {}", self.bid, self.ask, record.id);
//         self.book[price_id].push_back((record.size, record.id));
//
//         Ok(())
//     }
//
//     pub fn reload(&mut self, bids: Vec<Record>, asks: Vec<Record>) -> Result<()> {
//         self.bid = std::usize::MIN;
//         self.ask = std::usize::MAX;
//         self.book.iter_mut().map(|x| *x = VecDeque::new()).count();
//
//         self.update(bids, asks);
//
//         Ok(())
//     }
//
//     pub fn update(&mut self, bids: Vec<Record>, asks: Vec<Record>) -> Result<()> {
//         for bid in bids {
//             self.open(Side::Buy, bid).expect("Error while opening order");
//         }
//
//         for ask in asks {
//             self.open(Side::Sell, ask).expect("Error while opening order");
//         }
//
//         Ok(())
//     }
//
//     pub fn match_order(&mut self, price: f64, size: f64, id: Uuid) -> Result<()> {
//         let price_id = self.get_price_id(price)?;
//
//         if self.book[price_id].is_empty() || id != self.book[price_id][0].1 {
//             return Err(anyhow!("Coudln't match order with id {}", id));
//         }
//
//         let size_round = {
//             let scoped_size = &mut self.book[price_id][0].0;
//             *scoped_size -= size;
//             (*scoped_size * 1000.0).round()
//         };
//
//         if size_round == 0.0 {
//             self.book[price_id].pop_front();
//             self.check(price_id);
//         }
//
//         self.last_match = price_id;
//
//
//         Ok(())
//     }
//
//     pub fn finish(&mut self, price: f64, id: Uuid) -> Result<()> {
//         let price_id = self.get_price_id(price)?;
//
//         self.book[price_id].retain(|&(_, t_id)| {
//             t_id != id
//         });
//
//         self.check(price_id);
//
//         Ok(())
//     }
//
//     pub fn change_order(&mut self, price: f64, new_size: f64, id: Uuid) -> Result<()> {
//         let price_id = self.get_price_id(price)?;
//
//         if new_size == 0.0 {
//             self.finish(price, id)?;
//
//             return Ok(());
//         }
//
//         self.book[price_id].iter_mut().for_each(|(t_size, t_id)| {
//             if *t_id == id {
//                 *t_size = new_size;
//             }
//         });
//
//         Ok(())
//     }
//
//     fn check(&mut self, price_id: usize) {
//         if price_id == self.bid {
//             while self.book[self.bid].is_empty() {
//                 self.bid -= 1;
//             }
//         }
//
//         if price_id == self.ask {
//             while self.book[self.ask].is_empty() {
//                 self.ask += 1;
//             }
//         }
//     }
//
//     pub fn print_self(&self) {
//         println!("{}", self);
//     }
// }
//
// impl Display for OrderBook {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         if self.bid == std::usize::MIN || self.ask == std::usize::MAX {
//             return write!(f, "OB: empty");
//         }
//
//         let size = 10;
//         let round_lambda = |x: f64| (x*10000.0).round()/10000.0;
//
//         let bids = self.bids(size).into_iter().map(round_lambda).map(|x| x.to_string()).collect::<Vec<_>>().join(", ");
//         let asks = self.asks(size).into_iter().map(round_lambda).map(|x| x.to_string()).collect::<Vec<_>>().join(", ");
//         let bid = self.bid as f64 / 1000.0;
//         let ask = self.ask as f64 / 1000.0;
//
//         write!(f, "OB: {} | {:.3}   {:.3} | {}", bids, bid, ask, asks)
//     }
// }

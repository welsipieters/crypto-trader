use hashbrown::HashMap;
use crate::crypto::coin::Coin;
use crate::bot::trading::broker::Broker;

use std::sync::Arc;
use crate::crypto::orderbook::OrderBook;
use parking_lot::Mutex;
use tokio::sync::watch::Receiver;
use tokio::sync::mpsc::UnboundedSender;
use crate::crypto::treasury::TransactionIntent;

pub mod broker;

#[derive(Debug)]
pub struct Trader {
    brokers: HashMap<String, Broker>,
    receiver: Receiver<Tick>
}

impl Trader {
    pub fn new(receiver: Receiver<Tick>) -> Self {
        Self {
            brokers: HashMap::new(),
            receiver
        }
    }

    pub fn register_book<T: Into<String>>(&mut self, symbol: T, book: Arc<Mutex<OrderBook>>,
                                          intent_sender: UnboundedSender<TransactionIntent>) {
        let symbol = symbol.into();

        self.brokers.insert(symbol.clone(), Broker::new(
            symbol.clone(),
            book,
            self.receiver.clone(),
                intent_sender
        ));
    }

    pub fn start(&self) {
        for (_, broker) in self.brokers.iter() {
            broker.start();
        }
    }

}

#[derive(Debug)]
enum TradingEvent {
    BookUpdate,
    // In the future, I would like to extend these events.
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tick {
    Silent,
    Actionable,
    Output
}
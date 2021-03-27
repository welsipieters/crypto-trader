use anyhow::Result;
use async_trait::async_trait;
use hashbrown::HashMap;
use crate::exchanges::Exchange;
use crate::crypto::balances::BalanceMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::crypto::orderbook::{OrderSide, OrderType};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct Treasury {
    treasurers: HashMap<String, Arc<Treasurer>>
}

impl Treasury {
    pub fn new() -> Self {
        Self {
            treasurers: HashMap::new()
        }
    }

    pub fn treasure<T: Into<String>>(&mut self, exchange_id: T, exchange: Arc<Mutex<Box<dyn Exchange + Sync + Send>>>) -> UnboundedSender<TransactionIntent> {
        let (intent_sender, intent_receiver) = tokio::sync::mpsc::unbounded_channel();

        let treasurer = Arc::new(Treasurer::new(Arc::clone(&exchange)));
        treasurer.start_review_queue(intent_receiver);

        self.treasurers.insert(exchange_id.into(), Arc::clone(&treasurer));

        intent_sender
    }
}

pub struct Treasurer {
    exchange: Arc<Mutex<Box<dyn Exchange + Sync + Send>>>
}

impl Treasurer {
    pub fn new(exchange:  Arc<Mutex<Box<dyn Exchange + Sync + Send>>>) -> Self {
        Self { exchange }
    }

    pub fn review_transaction(&self, intent: &TransactionIntent) -> Option<ExecutableTransaction> {
        None
    }

    pub fn start_review_queue(&self, mut receiver: UnboundedReceiver<TransactionIntent>) {

    }

    fn allocate_funds() {

    }
}


#[derive(Debug)]
pub enum TransactionIntent {
    Buy {
        symbol: String,
        price: f64,
        meta: IntentMeta,
    },

    Sell {
        symbol: String,
        price: f64,
        amount: f64,
        meta: IntentMeta,
    }
}

#[derive(Debug)]
pub struct IntentMeta {
    pub existing_transaction: Option<String>
}


#[derive(Debug)]
pub enum ExecutableTransaction {
    Buy {
        symbol: String,
        price: f64,
        amount: f64,
        meta: TransactionMeta
    },

    Sell {
        symbol: String,
        price: f64,
        amount: f64,
        meta: TransactionMeta
    }
}

#[derive(Debug)]
pub struct TransactionMeta {
    pub existing_transaction: Option<String>
}


#[async_trait]
pub trait Treasured {
    fn request_balances(&self) -> &BalanceMap;
}
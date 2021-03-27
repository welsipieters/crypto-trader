use crate::bot::trading::{TradingEvent, Tick};
use std::sync::Arc;
use tokio::sync::{Notify};
use crate::crypto::orderbook::{OrderBook, OrderSide, OrderType};
use crate::CONFIG as Config;
use crate::utils::{count_transactions_for_pair, get_transactions_for_pair};
use crate::database::TransactionStage;
use parking_lot::Mutex;
use tokio::sync::watch::Receiver;
use std::borrow::BorrowMut;
use std::ops::Deref;
use crate::crypto::treasury::TransactionIntent;
use tokio::sync::mpsc::UnboundedSender;
use diesel::result::Error;
use crate::crypto::treasury::TransactionMeta;
use crate::crypto::treasury::IntentMeta;
use tokio::sync::mpsc::error::SendError;

#[derive(Debug)]
pub struct Broker {
    symbol: String,
    book: Arc<Mutex<OrderBook>>,
    receiver: Receiver<Tick>,
    intent_sender: UnboundedSender<TransactionIntent>
}

impl Broker {
    pub fn new<T: Into<String>>(symbol: T, book: Arc<Mutex<OrderBook>>, receiver: Receiver<Tick>, intent_sender: UnboundedSender<TransactionIntent>) -> Self {
        Self {
            symbol: symbol.into(),
            book,
            receiver,
            intent_sender
        }
    }

    pub fn symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn start(&self) {
        let book = Arc::clone(&self.book);
        let max_transactions = Config.max_transaction_per_coin;
        let coins = Config.coins.clone();
        let coin = coins.into_iter().find(|coin| &coin.symbol == &self.symbol).expect("Couldn't find coin in config");
        let support = coin.support;
        let increment = coin.profit_wanted;
        let lower = support - (support*(increment/2.0));
        let upper = support + (support*(increment/2.0));
        let symbol = self.symbol.clone();

        let mut receiver = self.receiver.clone();
        let intent_sender = self.intent_sender.clone();
        tokio::spawn(async move {
            loop {
                receiver.changed().await;
                let tick = *receiver.borrow();
                let book = book.lock();
                if let (Some(bid), Some(ask)) = (book.highest_bid(), book.lowest_ask()) {

                    if tick == Tick::Output {
                        info!("[Mandala]: Support for {} is configured at {}. Looking for a profit of {}%. (B: {:.3}, S: {:.3}) Current: (B: {:.4}, A: {:.4})", &symbol, &support, &increment, &lower, &upper, &bid, &ask);
                    }

                    if ask.as_ref() <= &lower {

                        let count = count_transactions_for_pair(
                            &symbol,
                            TransactionStage::open()
                        ).unwrap_or(0);

                        if count >= max_transactions {
                            continue;
                        }

                        match intent_sender.send(TransactionIntent::Buy {
                            symbol: symbol.clone(),
                            price: ask.as_ref().clone(),
                            meta: IntentMeta { existing_transaction: None }
                        }) {
                            Ok(_) => {}
                            Err(error) => {
                                info!("Error while sending intent: {:?}", error);
                            }
                        }
                    }

                    if bid >= upper {
                        match get_transactions_for_pair(&symbol, vec![TransactionStage::Hodl]) {
                            Ok(transactions) => {
                                // dbg!(&transactions);
                                if transactions.is_empty() {
                                    continue;
                                }

                                for transaction in transactions.iter() {
                                    if bid < transaction.price + (transaction.price * coin.profit_wanted) {
                                        // To prevent selling multiple transactions of one coin at a single price point.
                                        continue;
                                    }

                                    info!("[Mandala]: Found potential sell opportunity on {}. Price: {}", &symbol, &bid);
                                    info!("[Mandala]: THIS IS PROFIT.");
                                    info!("[Mandala]: selling {} of {} at {}", &transaction.amount, &symbol, &bid);



                                    if let Some(error) = intent_sender.send(TransactionIntent::Sell {
                                        symbol: symbol.clone(),
                                        price: bid.as_ref().clone(),
                                        amount: transaction.amount,
                                        meta: IntentMeta { existing_transaction: Some(transaction.id.clone()) }
                                    }).err() {
                                        dbg!(&error);
                                        panic!();
                                    }
                                }
                            }
                            Err(error) => {
                                error!("[Mandala]: Error getting transactions for {}: {:?}", symbol, error);
                            }
                        }
                    }
                }
            }
        });
    }
}
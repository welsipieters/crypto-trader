use crate::crypto::coin::Coin;
use crate::exchanges::mandala::utils::{
    DepthSnapshot, DepthUpdate, Order as MandalaOrder, WebsocketRequest,
};
use crate::exchanges::mandala::{BINANCE_API_URL, BINANCE_WSS_URL};
use crate::CONFIG;
use hashbrown::HashMap;
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Notify,
};

use tokio::time::{Duration, Instant};

use crate::crypto::orderbook::{Order, OrderBook, OrderSide};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::Ordering::Release;
use tokio_tungstenite::connect_async;
use tungstenite::Message;

use std::rc::Weak;
use hashbrown::hash_map::{Iter, DefaultHashBuilder};
use crate::crypto::treasury::TransactionIntent;

//
type Sender = UnboundedSender<DepthUpdate>;
type Receiver = UnboundedReceiver<DepthUpdate>;

pub struct Bookkeeper {
    coins: Vec<Coin>,
    bookies: HashMap<String, Bookie>,
}

impl Bookkeeper {
    pub fn new() -> Self {
        Self {
            coins: vec![],
            bookies: HashMap::new(),
        }
    }

    pub async fn boot(&mut self, coins: Vec<Coin>) {
        info!("[Mandala][Bookkeeper]: Booting...");

        for coin in coins.iter() {
            let book = Arc::new(Mutex::new(OrderBook::new(&coin.symbol)));
            let bookie = Bookie::new(&coin.symbol, Arc::clone(&book));

            self.bookies.insert(coin.symbol.clone(), bookie);
        }

        self.boot_websockets(&coins).await;
        futures::future::join_all(
            self.bookies
                .values_mut()
                .map(Bookie::boot)
                .collect::<Vec<_>>(),
        )
        .await;
    }

    async fn boot_websockets(&self, coins: &Vec<Coin>) {
        let quote_currency = &CONFIG.quote_currency;
        let params: Vec<_> = coins
            .into_iter()
            .map(|coin| format!("{}{}@depth@100ms", coin.symbol, quote_currency).to_lowercase())
            .collect();
        let request = WebsocketRequest::new(1, "SUBSCRIBE", params);
        let senders: HashMap<_, _> = self
            .bookies
            .iter()
            .map(|(coin, bookie)| (coin.clone(), bookie.get_sender()))
            .collect();

        let (stream, _) = connect_async(BINANCE_WSS_URL)
            .await
            .expect("Failed to connect");
        let (mut write, mut read) = stream.split();

        let json_request = serde_json::to_string(&request).expect("Error serializing subscription");
        write.send(Message::Text(json_request)).await.unwrap();

        // Handle first message
        if let Some(Ok(message)) = read.next().await {
            if let Message::Text(message) = message {
                Bookkeeper::handle_update(message, &senders).await;
            }
        } else {
            panic!("websocket receive faulted")
        }

        tokio::spawn(async move {
            while let Some(Ok(message)) = read.next().await {
                match message {
                    Message::Text(message) => Bookkeeper::handle_update(message, &senders).await,
                    Message::Binary(_) => {
                        info!("[Mandala]: Received Binary");
                    }
                    Message::Ping(_) => {
                        info!("[Mandala]: Received Ping");
                    }
                    Message::Pong(_) => {
                        info!("[Mandala]: Received Pong");
                    }
                    Message::Close(_) => {
                        info!("[Mandala]: Received Close");
                    }
                }
            }
        });
    }

    async fn handle_update(message: String, senders: &HashMap<String, Arc<Sender>>) {
        let update: Value = serde_json::from_str(message.as_str()).unwrap();

        if update.get("a").is_none() || update.get("b").is_none() {
            return;
        }

        let update: DepthUpdate = serde_json::from_value(update).expect("Invalid message");
        let symbol = update.symbol.clone().replace(&CONFIG.quote_currency, "");

        debug!("[Mandala][Bookkeeper]: Received update for {}", &symbol);

        match senders.get(&symbol) {
            None => {
                error!(
                    "[Mandala][Bookkeeper]: No bookie queue found for {}",
                    &symbol
                );
            }
            Some(sender) => {
                sender.send(update).unwrap();
            }
        }
    }

    pub fn sanity_check(&self) {
        for (symbol, bookie) in self.bookies.iter() {
            let book = bookie.book.lock();

            if book.lowest_ask() < book.highest_bid() {
                error!("Found lower ask than bid: {}", symbol);
                error!("[Mandala]: Top 5 asks and bids:");
                for (p, q) in book.asks.iter().take(5).rev() {
                    error!("@{:0<10}: {}", p, q)
                }
                error!("-------------");
                for (p, q) in book.bids.iter().take(5) {
                    error!("@{:0<10}: {}", p, q)
                }
            }
        }
    }

    pub fn get_book<T: Into<String>>(&mut self, coin: T) -> Option<Arc<Mutex<OrderBook>>> {
        match self.bookies.get(&coin.into()) {
            None => {
                None
            }
            Some(bookie) => {
                Some(Arc::clone(&bookie.book))
            }
        }
    }

    pub fn iter_bookies(&self) -> Iter<String, Bookie> {
        self.bookies.iter()
    }

    pub fn iter_books(&self) -> HashMap<String, Arc<Mutex<OrderBook>>> {
        self.bookies.iter().map(|(s ,b)| {
            (s.clone(), Arc::clone(&b.book))
        } ).collect()
    }
}

pub struct Bookie {
    symbol: String,
    can_process: Arc<Notify>,
    last_update_id: Arc<AtomicI64>,
    book: Arc<Mutex<OrderBook>>,
    sender: Arc<Sender>
}

impl Bookie {
    pub fn new<T: Into<String>>(symbol: T, book: Arc<Mutex<OrderBook>>) -> Self {
        let (sender, receiver) = unbounded_channel();

        let mut bookie = Self {
            symbol: symbol.into(),
            can_process: Arc::new(Notify::new()),
            last_update_id: Arc::new(AtomicI64::new(0)),
            book,
            sender: Arc::new(sender),
        };

        bookie.start(receiver);
        bookie
    }

    fn start(&mut self, mut receiver: Receiver) {
        let symbol_name = format!("{}_{}", self.symbol, CONFIG.quote_currency);
        info!("[Mandala][Bookie]: Starting bookie for {}", &symbol_name);

        let mut can_process = Arc::clone(&self.can_process);
        let mut last_update = Arc::clone(&self.last_update_id);
        let book = Arc::clone(&self.book);

        tokio::spawn(async move {
            can_process.notified().await;

            while let Some(update) = receiver.recv().await {
                if update.last_id < last_update.load(Ordering::Relaxed) {
                    continue;
                }

                debug!("[Mandala][Bookie]: Handled update for {}", &symbol_name);

                last_update.store(update.last_id.clone(), Release);

                let mut bids = update
                    .bids
                    .iter()
                    .map(|order| Self::convert_record(order, OrderSide::Buy))
                    .collect::<Vec<_>>();

                let mut asks = update
                    .asks
                    .iter()
                    .map(|order| Self::convert_record(order, OrderSide::Sell))
                    .collect::<Vec<_>>();

                book.as_ref().lock().update(bids, asks);
            }
        });
    }

    pub async fn boot(&mut self) {
        let symbol = format!("{}{}", &self.symbol, CONFIG.quote_currency);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let result = reqwest::get(format!(
            "{}/depth?symbol={}&limit=1000",
            BINANCE_API_URL,
            symbol.to_uppercase()
        )).await;

        match result {
            Ok(response) => {
                let snapshot: DepthSnapshot = response.json().await.expect("Invalid json");

                self.last_update_id
                    .store(snapshot.last_update_id.clone(), Release);

                let mut bids = snapshot
                    .bids
                    .iter()
                    .map(|order| Self::convert_record(order, OrderSide::Buy))
                    .collect::<Vec<_>>();

                let mut asks = snapshot
                    .asks
                    .iter()
                    .map(|order| Self::convert_record(order, OrderSide::Sell))
                    .collect::<Vec<_>>();

                self.book.as_ref().lock().reload(bids, asks);

                debug!(
                    "[Mandala][Bookie]: Finished processing snapshot for {}, unlocking...",
                    format!("{}_{}", &self.symbol, CONFIG.quote_currency),
                );

                self.can_process.notify_waiters();
            }
            Err(error) => {
                error!(
                    "[Mandala][Bookie]: Error while requesting depth snapshot for {}: {:?}",
                    format!("{}_{}", &self.symbol, CONFIG.quote_currency),
                    error
                );
            }
        }
    }

    pub fn get_sender(&self) -> Arc<Sender> {
        Arc::clone(&self.sender)
    }

    fn convert_record(order: &MandalaOrder, side: OrderSide) -> Order {
        Order::new(side, order.1, order.0)
    }
}
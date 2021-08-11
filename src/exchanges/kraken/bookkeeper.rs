use serde::{Serialize, Deserialize};
use hashbrown::HashMap;
use crate::crypto::coin::Coin;
use tokio::sync::{RwLock, Notify};
use crate::crypto::orderbook::OrderBook;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded_channel};
use serde_json::Value;
use crate::CONFIG;
use std::collections::BTreeMap;
use tokio_tungstenite::connect_async;
use futures_util::{StreamExt, SinkExt};
use tungstenite::Message;
use crate::exchanges::kraken::Asset;

type Sender = UnboundedSender<Value>;
type Receiver = UnboundedReceiver<Value>;

pub struct Bookkeeper {
    coins: Vec<Coin>,
    bookies: HashMap<String, Bookie>
}

impl Bookkeeper {
    pub fn new() -> Bookkeeper {
        Self {
            coins: vec![],
            bookies: Default::default()
        }
    }

    pub async fn boot(&mut self, assets: &Vec<Asset>) {
        info!("[Kraken][Bookkeeper]: Booting...");

        for coin in assets.iter() {
            let book = Arc::new(RwLock::new(OrderBook::new(&coin.symbol)));
            let bookie = Bookie::new(&coin.symbol, Arc::clone(&book));

            self.bookies.insert(coin.symbol.clone(), bookie);
        }

        self.boot_websockets(assets)
    }

    async fn boot_websockets(&self, coins: &Vec<Asset>) {
        let quote_currency = &CONFIG.quote_currency;

        let pairs: Vec<_> = coins.into_iter().map(|coin| { coin.wsname }).collect();
        let senders: HashMap<_, _> = self.bookies.iter().map(|(coin, bookie)| (coin.clone(), bookie.get_sender())).collect();

        let (stream, _) = connect_async(
            CONFIG.kraken.wss_url.clone()
        ).await.expect("Error establishing websocket connection with Kraken");

        let (mut write, mut read) = stream.split();

        let subscription_message = Subscription {
            event: "subscribe".to_string(),
            pair: pairs,
            subscription: SubObj {
                depth: 500,
                interval: 1,
                name: "book".to_string(),
                token: "xyz".to_string()
            }
        };
        let json_subscription = serde_json::to_string(&subscription_message).expect("Error serializing websocket subscription");

        write.send(Message::Text(json_subscription)).await.expect("Error submitting subscription");

         // Handle the first message. This message is a snapshot. NOT an update.

        if let Some(Ok(message)) = read.next().await {
            if let Message::Text(message) = message {
                // TODO: Handle snapshot.
                dbg!(message);
            }
        } else {
            panic!("[Kraken][Bookkeeper]: Websocket receive faulted.");
        }
        
        tokio::spawn(async move {
            while let Some(Ok(message)) = read.next().await {
                match message {
                    .. => debug!("[Kraken][Bookkeeper]: Received unknown."),
                    Message::Ping(_) => info!("[Kraken][Bookkeeper]: Received Ping"),
                    Message::Pong(_) => info!("[Kraken][Bookkeeper]: Received Pong"),
                    Message::Close(_) => info!("[Kraken][Bookkeeper]: Received Close"),
                    Message::Text(_) => {
                        // TODO: handle update
                    }
                }
            }
        });
    }
}

struct Bookie {
    symbol: String,
    can_process: Arc<Notify>,
    book: Arc<RwLock<OrderBook>>,
    sender: Arc<Sender>
}

impl Bookie {
    pub fn new<T: Into<String>>(symbol: T, book: Arc<RwLock<OrderBook>>) -> Bookie {
        let (sender, receiver) = unbounded_channel();

        let mut bookie = Self {
            symbol: symbol.into(),
            can_process: Arc::new(Notify::new()),
            book,
            sender: Arc::new(sender)
        };

        bookie
    }

    pub async fn boot(&mut self) {

    }

    pub async fn start(&mut self, mut receiver: Receiver) {
        let symbol_name = format!("{}_{}", self.symbol, CONFIG.quote_currency);

        info!("[Kraken][Bookie]: Starting bookie for {}", &symbol_name);
        let can_process = Arc::clone(&self.can_process);
        let book = Arc::clone(&self.book);

        tokio::spawn(async move {
            can_process.notified().await;

            while let Some(msg) = receiver.recv().await {
                dbg!(msg)
            }
        });
    }

    pub fn get_sender(&self) -> Arc<Sender> {
        Arc::clone(&self.sender)
    }

}
#[derive(Clone, Copy, Debug, Deserialize)]
struct Subscription {
    event: String,
    pair: Vec<String>,
    subscription: SubObj
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct SubObj {
    depth: i32,
    interval: i32,
    name: String,
    token: String
}



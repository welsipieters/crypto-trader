use crate::crypto::balances::{BalanceMap, Balance};
use crate::crypto::coin::Coin;
use crate::crypto::{Fees};
use crate::database::{Transaction, TransactionStage, UpdateTransactionStageForm};
use crate::exchanges::mandala::bookkeeper::Bookkeeper;
use crate::exchanges::mandala::client::Client;
use crate::exchanges::mandala::utils::{DepthUpdate, ListedResponse, MandalaResponse, Order, OrderStatus, RequestedOrder, Symbol, WebsocketRequest, OrderType, OrderSide, OrderRequest, PlaceOrderResponse, AccountInfo};
use crate::exchanges::Exchange;
use crate::CONFIG;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, QueryResult, RunQueryDsl};
use hashbrown::HashMap;
use hmac::Hmac;
use minreq::{Method, Response, Error};
use parking_lot::Mutex;
use serde_json::Value;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::sync::Arc;
use websocket_lite::{ClientBuilder, Message, Opcode};
use crate::bot::trading::{Trader, Tick};
use tokio::sync::Notify;
use crate::bot::trading::broker::Broker;
use crate::crypto::treasury::{Treasured, TransactionIntent, ExecutableTransaction};
use tokio::sync::watch::Sender;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use round::round_down;


mod bookkeeper;
mod client;
pub mod utils;

const DEFAULT_RECV_WINDOW: usize = 5000;
const MANDALA_API_URL: &str = "https://trade.mandala.exchange";
const MANDALA_WSS_URL: &str = "wss://trade.mandala.exchange/ws";
const BINANCE_API_URL: &str = "https://api.binance.com/api/v3";
const BINANCE_WSS_URL: &str = "wss://stream.binance.com:9443/ws";

pub struct Mandala {
    client: Client,
    bookkeeper: Bookkeeper,
    balances: BalanceMap,
    trader: Trader,
    trader_sender: Sender<Tick>,
}

impl Mandala {
    pub fn new() -> Self {
        let api_key = CONFIG.mandala.api_key.clone();
        let api_secret = CONFIG.mandala.api_secret.clone();
        let (trader_sender, trader_receiver) =  tokio::sync::watch::channel(Tick::Output);

        Self {
            client: Client::new(api_key, api_secret),
            bookkeeper: Bookkeeper::new(),
            balances: BalanceMap::new(),
            trader: Trader::new(trader_receiver.clone()),
            trader_sender,
          }
    }

    fn spawn_brokers(&mut self, intent_sender: UnboundedSender<TransactionIntent>) {
        for (symbol, book) in self.bookkeeper.iter_books() {
            self.trader.register_book(
                &symbol,
                book,
                intent_sender.clone()
            );
        }
    }

    pub fn get_open_orders(&self) -> QueryResult<Vec<Transaction>> {
        let search_stage = TransactionStage::open()
            .into_iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>();

        use crate::schema::transactions::dsl::*;

        let connection = crate::DATABASE.get_connection();
        transactions
            .filter(exchange_name.eq(self.get_identifier()))
            .filter(stage.eq_any(search_stage))
            .load::<Transaction>(&connection)
    }

    fn reload_balances(&mut self) {
        info!("Reloading balances");
        let endpoint = "/open/v1/account/spot";
        let result = self.client.request(minreq::Method::Get, endpoint, BTreeMap::new(), true);

        match result {
            Ok(response) => {
                let response: MandalaResponse<AccountInfo> = response.json().expect("Error deserializing json");
                let balances = response.data.account_assets.iter().map(|a|  {
                    let balance = Balance::new(
                        a.asset.clone(),
                        a.free,
                        a.locked
                    );

                    (a.asset.clone(), balance)
                }).collect::<HashMap<_, _>>();

                self.balances.reload(balances);
            }
            Err(error) => {
                error!("[Mandala]: Error while requesting balances: {}", error.to_string())
            }
        }

    }
}

#[async_trait]
impl Exchange for Mandala {
    async fn boot(&mut self, intent_sender: UnboundedSender<TransactionIntent>) {
        info!("[Mandala]: Booting...");

        let result = self.client.request(
            Method::Get,
            "/open/v1/common/symbols",
            BTreeMap::new(),
            false,
        );

        match result {
            Ok(response) => {
                let symbols: MandalaResponse<ListedResponse<Symbol>> =
                    response.json().expect( "no json");
                let mut tradable_coins = vec![];

                info!("[Mandala]: Found {} products", symbols.data.list.len());

                for symbol in symbols.data.list.iter() {
                    // debug!("[Mandala]: Checking pair {} for tradability.", &symbol.symbol);

                    let coins = CONFIG.coins.clone();
                    let matching = coins.into_iter().find(|coin| {
                        coin.symbol == symbol.base_currency
                            && symbol.quote_currency == CONFIG.quote_currency
                    });

                    if matching.is_none() {
                        // debug!("[Mandala]: Identified pair {} as non-tradable.", &symbol.symbol);

                        continue;
                    }

                    if symbol.symbol_type != 1 {
                        error!("The pair {} is NOT routed through the Binance order books and runs directly on Mandala. This is currently unsupported.", symbol.symbol);

                        continue;
                    }

                    if tradable_coins.contains(&Coin::new(&symbol.base_currency)) {
                        error!(
                            "[Mandala]: Tried to register pair {} multiple times.",
                            &symbol.symbol
                        );

                        continue;
                    }

                    info!("[Mandala]: Identified pair {} as tradable", &symbol.symbol);

                    tradable_coins.push(Coin::new(symbol.base_currency.clone()));
                }

                self.bookkeeper.boot(tradable_coins).await;
                self.spawn_brokers(intent_sender);
            }
            Err(error) => {
                error!("[Mandala]: Error while fetching symbols: {:?}", error);
            }
        }

        self.trader.start();
    }

    fn get_identifier(&self) -> String {
        "mandala".to_string()
    }

    fn get_display_name(&self) -> String {
        "Mandala".to_string()
    }

    async fn tick(&mut self, debug: bool, actionable: bool) {

        let mut tick = Tick::Silent;

        if debug {
            info!("---");
            tick = Tick::Output;
            self.bookkeeper.sanity_check();
        }

        if actionable {
            tick = Tick::Actionable;

            self.reload_balances();
            self.check_open_orders();
        }

        self.trader_sender.send(tick).expect("Error");
    }

    fn balances(&self) -> &BalanceMap {
        &self.balances
    }

    fn get_fees(&self) -> &Fees {
        unimplemented!()
    }

    fn check_open_orders(&self) {
        let open_orders = self.get_open_orders().expect("no open orders");

        for order in open_orders.into_iter() {
            tokio::spawn(async move {
                let endpoint = "/open/v1/orders/detail";
                let mut params = BTreeMap::new();
                let mut sell = false;
                if order.sell_exchange_id.is_some() {
                    sell = true;
                    params.insert(
                        "orderId".into(),
                        order.sell_exchange_id.as_ref().unwrap().clone(),
                    );
                } else {
                    params.insert(
                        "orderId".into(),
                        order.buy_exchange_id.as_ref().unwrap().clone(),
                    );
                }

                let mut param_string = Client::create_param_string(params);
                param_string = Client::sign_params(param_string);
                let client = reqwest::Client::new();

                let result = client.get(format!("{}{}?{}", MANDALA_API_URL, endpoint, param_string))
                    .header("X-MBX-APIKEY", &CONFIG.mandala.api_key).send().await;


                match result {
                    Ok(response) => {
                        let response: MandalaResponse<RequestedOrder> =
                            response.json().await.expect("Invalid json");
                        let mut stage = TransactionStage::BuyTransactionOpen;
                        let mut amount = order.amount;

                        if sell {
                            stage = TransactionStage::SellTransactionOpen;
                        }

                        if response.data.status == OrderStatus::PartiallyFilled {
                            stage = TransactionStage::BuyTransactionPartiallyFilled;

                            if sell {
                                stage = TransactionStage::SellTransactionPartiallyFilled;
                            }
                        }

                        if response.data.status == OrderStatus::Filled {
                            stage = TransactionStage::Finished;

                            if !sell {
                                stage = TransactionStage::Hodl;
                                amount = (response.data.executed_quantity * 0.99);
                            }
                        }

                        let change_set = UpdateTransactionStageForm {
                            stage: stage.to_string(),
                            sell_exchange_id: None,
                            updated_at: Some(Utc::now().naive_utc()),
                            amount
                        };

                        if order.stage != stage.to_string() {
                            info!(
                                "[Mandala]: Updating status for order {} from {} to {}",
                                order.id,
                                order.stage,
                                stage.to_string()
                            );
                        }

                        let connection = crate::DATABASE.get_connection();
                        diesel::update(&order).set(change_set).execute(&connection);
                    }
                    Err(e) => {
                        error!(
                            "[Mandala]: Error while checking transaction {}: {}",
                            &order.id,
                            e.to_string()
                        );
                    }
                }
            });
        }
    }

    fn execute_transaction(&mut self, transaction: &ExecutableTransaction) -> Result<String> {
        let endpoint = "/open/v1/orders";

        let request = match transaction {
            ExecutableTransaction::Buy {
                price,
                amount,
                symbol,
                ..
            } => {
                OrderRequest::new(
                    format!("{}_{}", symbol, CONFIG.quote_currency),
                    OrderSide::Buy,
                    Some(*amount),
                    Some(*price)
                )
            }

            ExecutableTransaction::Sell {
                price,
                amount,
                symbol,
                ..
            } => {
                OrderRequest::new(
                    format!("{}_{}", symbol, CONFIG.quote_currency),
                    OrderSide::Sell,
                    Some(*amount),
                    Some(*price)
                )
            }
        };
        let params = request.to_map();

        let result = self.client.request(
            Method::Post,
            endpoint,
            params,
            true
        );

        match result {
            Ok(response) => {
                let response: MandalaResponse<PlaceOrderResponse> = response.json()?;
                self.reload_balances();

                Ok(format!("{}", response.data.order_id))
            }
            Err(error) => {
                error!("[Mandala]: Error while executing order: {:?}", error);

                Err(anyhow!(error))
            }
        }


    }
}

impl Treasured for Mandala {
    fn request_balances(&self) -> &BalanceMap {
        &self.balances
    }
}
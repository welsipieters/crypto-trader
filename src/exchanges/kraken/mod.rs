use async_trait::async_trait;
use crate::exchanges::kraken::client::{Client, MessageType};
use crate::CONFIG;
use crate::exchanges::kraken::bookkeeper::Bookkeeper;
use crate::crypto::balances::{BalanceMap, Balance};
use std::collections::BTreeMap;
use hashbrown::HashMap;
use reqwest::{Response, Error};
use crate::exchanges::Exchange;
use crate::crypto::treasury::{ExecutableTransaction, TransactionIntent, Treasured};
use tokio::sync::mpsc::UnboundedSender;
use crate::crypto::Fees;
use crate::bot::trading::Tick;
use anyhow::Result;
use crate::schema::transactions::dsl::symbol;
use itertools::Itertools;
use serde::Deserialize;
use serde_json::Value;


mod client;
mod bookkeeper;

pub struct Kraken {
    client: Client,
    bookkeeper: Bookkeeper,
    balances: BalanceMap
}

impl Kraken {
    pub fn new() -> Self {
        let api_key = CONFIG.kraken.api_key.clone();
        let api_secret = CONFIG.kraken.api_secret.clone();

        Self {
            client: Client::new(api_key, api_secret),
            bookkeeper: Bookkeeper::new(),
            balances: BalanceMap::new()
        }
    }

    async fn reload_balances(&mut self) {
        info!("[Kraken][Core]: Reloading balances");

        let endpoint = "/private/Balance";
        let result = self.client.send(
            MessageType::Private,
            endpoint,
            BTreeMap::new()
        ).await;

        match result {
            Ok(response) => {
                let response: KrakenResult<HashMap<String, String>> = response.json().await.expect("Error deserializing json");

                let balances = response.result.iter().map(|(symbol2, balance)| {
                    let balance = Balance::new(
                        symbol2,
                        balance.parse().expect("can't parse balance"),
                        0.0
                    );

                    (symbol2.clone(), balance)
                }).collect::<HashMap<_, _>>();

                self.balances.reload(balances);
            },
            Err(error) => {
                error!("[Kraken][Core]: Error while fetching balanes. {:?}", error);
            }
        };
    }

    pub async fn get_tradable_pairs(&self) -> Result<HashMap<String, Asset>> {
        match self.client.send(MessageType::Public, "/0/public/AssetPairs", BTreeMap::new()).await {
            Ok(response) => {
                // let value: Value = response.json().await.unwrap();
                // dbg!(value);
                // panic!();
                let result: KrakenResult<HashMap<String, Asset>> = response.json().await.expect("Cant get pairs");

                Ok(result.result)
            }
            Err(e) => {
                error!("Error while getting pairs: {:?}", e);

                Err(anyhow!("error while getting pairs."))
            }
        }
    }
}

#[derive(Eq, PartialEq, Debug, Deserialize)]
pub struct Asset {
    altname: String,
    wsname: Option<String>,
    base: String,
    quote: String
}

impl Asset {

}


#[async_trait]
impl Exchange for Kraken {
    async fn boot(&mut self, intent_sender: UnboundedSender<TransactionIntent>) {
        info!("[Kraken][Core]: Booting");

        let pairs = self.get_tradable_pairs().await.expect("cant get pairs");

        info!("[Kraken][Core]: Found {} markets", pairs.len());

        let mut tradable = vec![];

        for (key, asset) in pairs {
            if (asset.wsname.is_none()) { continue; }
            info!("[Kraken][Core]: Checking pair {} for tradability", asset.wsname.clone().expect("no wsname"));

            let coins = CONFIG.coins.clone();

            let matching = coins.into_iter().find(|c| {
                // dbg!(&c, &asset);
                c.symbol == asset.base && asset.quote == CONFIG.quote_currency
            });

            if matching.is_none() { continue; }

            if tradable.contains(&asset) {
                error!("[Kraken][Core]: Tried to register {} more then once.", asset.wsname.clone().expect("no wsname"));

                continue;
            }

            info!("[Kraken][Core]: Identified pair {} as tradable", asset.wsname.clone().expect("no wsname"));
            tradable.push(asset);
        }

        self.bookkeeper.boot(&tradable).await;
    }

    fn get_identifier(&self) -> String {
        "kraken".to_string()
    }

    fn get_display_name(&self) -> String {
        "Kraken".to_string()
    }

    async fn tick(&mut self, debug: bool, actionable: bool) {
        let mut tick = Tick::Silent;

        if debug {
            info!("[Kraken][Core]: --- [ROUND INFO] ---");
            tick = Tick::Output;
        }

        if actionable {
            tick = Tick::Actionable;

            // TODO: Reload balances, check open orders, etc. This is the modifying part of the game loop. so to speak.
        }
    }

    fn balances(&self) -> &BalanceMap {
        self.balances()
    }

    fn get_fees(&self) -> &Fees {
        todo!()
    }

    fn check_open_orders(&self) {
        todo!()
    }

    fn execute_transaction(&mut self, transaction: &ExecutableTransaction) -> Result<String> {
        Ok("test ser".to_string())
    }
}

#[derive(Deserialize)]
struct KrakenResult<T> {
    error: Vec<String>,
    result: T
}

impl Treasured for Kraken {
    fn request_balances(&self) -> &BalanceMap {
        &self.balances
    }
}
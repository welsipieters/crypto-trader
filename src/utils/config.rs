use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Error};

use std::env;

#[derive(Deserialize)]
pub struct Config {
    pub quote_currency: String,
    pub max_trade_size: f64,
    pub min_trade_size: f64,
    pub max_transaction_per_coin: i64,
    pub coins: Vec<Coin>,
    pub mandala: MandalaConfig,
    pub kraken: KrakenConfig,
    pub database_url: String,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
pub struct Coin {
    pub symbol: String,
    pub support: f64,
    pub profit_wanted: f64,
}

#[derive(Deserialize)]
pub struct MandalaConfig {
    pub enabled: bool,
    pub api_key: String,
    pub api_secret: String,
}


#[derive(Deserialize)]
pub struct KrakenConfig {
    pub enabled: bool,
    pub rest_url: String,
    pub wss_url: String,
    pub api_key: String,
    pub api_secret: String,
}

impl Config {
    pub fn load() -> Self {
        info!("Reading config");
        let path = env::current_dir().unwrap();
        let path = path.as_os_str().to_str().unwrap();
        let file_path = format!("{}/config.json", path.to_string());
        dbg!(&file_path);

        match File::open(file_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                match serde_json::from_reader(reader) {
                    Ok(config) => return config,
                    Err(e) =>{ dbg!(e); panic!()}
                }
            }
            Err(err) => {
                panic!(err)
            }
        };


        panic!("Couldn't read config file.");
    }
}

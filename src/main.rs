#![feature(async_closure)]
#![feature(atomic_from_mut)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate lazy_static;


use crate::bot::Poppy;
use crate::database::DatabaseManager;
use crate::exchanges::mandala::Mandala;
use crate::utils::config::Config;
use crate::exchanges::kraken::Kraken;

mod bot;
mod crypto;
mod database;
mod exchanges;
mod schema;
mod utils;

lazy_static! {
    pub static ref CONFIG: Config = { Config::load() };
    pub static ref DATABASE: DatabaseManager = { DatabaseManager::new() };
}

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting Poppy...");

    let mut mandala = Mandala::new();
    let mut kraken = Kraken::new();

    let mut poppy = Poppy::new();
    // poppy.register_exchange(Box::new(mandala)).await;
    poppy.register_exchange(Box::new(kraken)).await;

    poppy.run().await;
}

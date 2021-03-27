use chrono::NaiveDateTime;
use diesel::r2d2::ConnectionManager;
use diesel::sql_types::Text;
use diesel::MysqlConnection;
use r2d2::{Pool, PooledConnection};
use serde::{Deserialize, Serialize};
use std::fmt;

use diesel::prelude::*;

use crate::schema::{finished_transactions, transactions};

pub struct DatabaseManager {
    pool: Pool<ConnectionManager<MysqlConnection>>,
}

impl DatabaseManager {
    pub fn new() -> Self {
        let connection_manager =
            ConnectionManager::<MysqlConnection>::new(Self::get_database_url());
        let pool = Pool::builder()
            .build(connection_manager)
            .expect("Error while creating DB connection pool");

        Self { pool }
    }

    pub fn get_connection(&self) -> PooledConnection<ConnectionManager<MysqlConnection>> {
        self.pool.get().expect("Failed to get connection from pool")
    }

    pub fn get_database_url() -> String {
        crate::CONFIG.database_url.clone()
    }
}

#[derive(Queryable, Identifiable, Serialize, Deserialize, Debug, Insertable)]
#[table_name = "transactions"]
pub struct Transaction {
    pub id: String,
    pub exchange_name: String,
    pub buy_exchange_id: Option<String>,
    pub sell_exchange_id: Option<String>,
    pub amount: f64,
    pub symbol: String,
    pub price: f64,
    pub stage: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(AsChangeset)]
#[table_name = "transactions"]
pub struct UpdateTransactionStageForm {
    pub stage: String,
    pub sell_exchange_id: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub amount: f64,
}

#[derive(Queryable, Identifiable, Serialize, Deserialize, Debug, Insertable)]
#[table_name = "finished_transactions"]
pub struct FinishedTransaction {
    pub id: String,
    pub transaction_id: String,
    pub amount_bought: f64,
    pub buy_price: f64,
    pub amount_sold: f64,
    pub sell_price: f64,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Deserialize, Serialize, Debug, AsExpression, FromSqlRow)]
#[sql_type = "Text"]
pub enum TransactionStage {
    BuyTransactionOpen,
    BuyTransactionPartiallyFilled,
    BuyTransactionFilled,
    Hodl,
    SellTransactionOpen,
    SellTransactionPartiallyFilled,
    SellTransactionFilled,
    Finished,
}

impl TransactionStage {
    pub fn open() -> Vec<Self> {
        vec![
            Self::BuyTransactionOpen,
            Self::BuyTransactionPartiallyFilled,
            Self::BuyTransactionFilled,
            Self::Hodl,
            Self::SellTransactionOpen,
            Self::SellTransactionPartiallyFilled,
        ]
    }
}

impl fmt::Display for TransactionStage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

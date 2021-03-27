pub mod config;

use barrel::Migration;
use serde::de::Unexpected;
use serde::{de, Deserialize, Deserializer};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::database::{TransactionStage, Transaction};
use diesel::{QueryDsl, ExpressionMethods, QueryResult};
use crate::diesel::RunQueryDsl;
use diesel::dsl::count;
use crate::exchanges::Exchange;
use crate::crypto::treasury::Treasured;

fn test(_migr: &mut Migration) {}

pub fn get_timestamp() -> u64 {
    let start = SystemTime::now();
    let since_epoch = start.duration_since(UNIX_EPOCH).expect("Error");

    since_epoch.as_secs() * 1000 + u64::from(since_epoch.subsec_nanos()) / 1_000_000
}

pub fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    match u8::deserialize(deserializer)? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(de::Error::invalid_value(
            Unexpected::Unsigned(other as u64),
            &"zero or one",
        )),
    }
}

pub fn f64_from_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => s.parse().map_err(de::Error::custom)?,
        Value::Number(num) => num.as_f64().ok_or(de::Error::custom("Invalid number"))?,
        _ => return Err(de::Error::custom("wrong type")),
    })
}

pub fn count_transactions_for_pair<T: Into<String>>(search_symbol: T, search_stage: Vec<TransactionStage>) -> Result<i64, diesel::result::Error> {
    let search_stage = search_stage.into_iter().map(|x| x.to_string()).collect::<Vec<_>>();
    use crate::schema::transactions::dsl::*;

    let connection = crate::DATABASE.get_connection();
    transactions
        .filter(stage.eq_any(search_stage))
        .filter(symbol.eq(search_symbol.into()))
        .select(count(id))
        .first(&connection)
}

pub fn get_transactions_for_pair<T: Into<String>>(search_symbol: T, search_stage: Vec<TransactionStage>) -> QueryResult<Vec<Transaction>> {
    let search_stage = search_stage.into_iter().map(|x| x.to_string()).collect::<Vec<_>>();
    use crate::schema::transactions::dsl::*;

    let connection = crate::DATABASE.get_connection();
    transactions
        .filter(stage.eq_any(search_stage))
        .filter(symbol.eq(search_symbol.into()))
        .load::<Transaction>(&connection)
}

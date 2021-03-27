use crate::crypto::balances::BalanceMap;
use crate::crypto::Fees;
use crate::database::{FinishedTransaction, TransactionStage};
use anyhow::Result;
use async_trait::async_trait;
use diesel::{QueryDsl, RunQueryDsl};
use hashbrown::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::crypto::treasury::{Treasured, TransactionIntent, ExecutableTransaction};
use tokio::sync::mpsc::UnboundedSender;

pub mod mandala;


#[async_trait]
pub trait Exchange: Treasured {
    async fn boot(&mut self, intent_sender: UnboundedSender<TransactionIntent>);
    fn get_identifier(&self) -> String;
    fn get_display_name(&self) -> String;
    async fn tick(&mut self, debug: bool, actionable: bool);
    fn balances(&self) -> &BalanceMap;
    fn get_fees(&self) -> &Fees;
    fn check_open_orders(&self);
    fn get_orders(&self, symbol: Option<String>, stages: Option<Vec<TransactionStage>>) {}
    fn execute_transaction(&mut self, transaction: &ExecutableTransaction) -> Result<String>;
}

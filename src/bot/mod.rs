use crate::exchanges::Exchange;
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use hashbrown::HashMap;
use crate::crypto::treasury::{Treasury, Treasured, TransactionIntent, ExecutableTransaction, TransactionMeta};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use crate::CONFIG as Config;
use crate::crypto::balances::Balance;
use anyhow::Error;
use crate::database::{Transaction, TransactionStage, UpdateTransactionStageForm, FinishedTransaction};
use crate::schema::transactions::columns::{symbol, id};
use chrono::Utc;
use diesel::{RunQueryDsl, QueryDsl, ExpressionMethods};
use crate::schema::transactions::dsl::transactions;
use uuid::Uuid;
use round::round_down;

pub mod trading;

pub struct Poppy {
    exchanges: HashMap<String, Arc<Mutex<Box<dyn Exchange + Sync + Send>>>>,
}

impl Poppy {
    pub fn new() -> Self {
        Self { exchanges: HashMap::new() }
    }

    pub async fn register_exchange(&mut self, mut exchange: Box<dyn Exchange + Send + Sync>)
    {
        if self
            .exchanges
            .contains_key(exchange.get_identifier().as_str())
        {
            panic!("Exchange {} registered twice", exchange.get_identifier());
        }

        info!("Registering exchange: {}", exchange.get_display_name());

        let (intent_sender, intent_receiver) = unbounded_channel();
        exchange.boot(intent_sender).await;

        let identifier = exchange.get_identifier().clone();
        let exchange = Arc::new(Mutex::new(exchange));

        self.spawn_intent_handler(&exchange, intent_receiver);
        self.exchanges.insert(identifier, exchange);
    }

    fn spawn_intent_handler(&self, exchange: &Arc<Mutex<Box<dyn Exchange + Sync + Send>>>, mut intent_receiver: UnboundedReceiver<TransactionIntent>) {
        let exchange = Arc::clone(exchange);

        tokio::spawn(async move {
           while let Some(intent) = intent_receiver.recv().await {
                let mut exchange = exchange.lock().await;

                let executable = match intent {
                    TransactionIntent::Buy {
                        symbol: tx_symbol,
                        price,
                        meta
                    } => {

                        let min_quote_order_size = Config.min_trade_size;
                        let mut quote_order_size = Config.max_trade_size;

                        match exchange
                            .balances()
                            .get_balance_for_symbol(&Config.quote_currency) {
                            None => {
                                panic!("Invalid quote currency.");
                            }
                            Some(balance) => {
                                while quote_order_size > balance.available {
                                    quote_order_size -= (quote_order_size * 0.01);
                                }
                            }
                        }

                        if quote_order_size < min_quote_order_size {
                            continue;
                        }

                        let mut amount = round_down(&quote_order_size / price, 2);

                        if tx_symbol == "STMX" {
                            // TODO: Why the fuck does binance do this and can we find a list of coins for which this is active?????
                            amount = amount.floor();
                        }

                        info!("[{}]: Found buy opportunity on {}. Price: {}", &exchange.get_identifier(), &tx_symbol, &price);
                        info!("[{}]: Buying {} of {} at {}", &exchange.get_identifier(), &amount, &tx_symbol, &price);

                        ExecutableTransaction::Buy {
                            symbol: tx_symbol,
                            price,
                            amount,
                            meta: TransactionMeta {
                                existing_transaction: meta.existing_transaction
                            }
                        }
                    }
                    TransactionIntent::Sell {
                        symbol: tx_symbol,
                        price,
                        amount,
                        meta
                    } => {
                        ExecutableTransaction::Sell {
                            symbol: tx_symbol,
                            price,
                            amount,
                            meta: TransactionMeta {
                                existing_transaction: meta.existing_transaction
                            }
                        }
                    }
                };

               let exchange_id = exchange.get_identifier().clone();
                match exchange.execute_transaction(&executable) {
                    Ok(tx_id) => {
                        match executable {
                            ExecutableTransaction::Buy {
                                symbol: tx_symbol,
                                amount,
                                price,
                                ..
                            } => {
                                Self::record_transaction_to_database(
                                    tx_symbol.clone(),
                                    exchange_id.clone(),
                                    tx_id.clone(),
                                    amount,
                                    price
                                );
                            }
                            ExecutableTransaction::Sell {
                                meta,
                                amount,
                                price,
                                ..
                            } => {
                                Self::update_transaction_for_sale(
                                    meta.existing_transaction.expect("No existing transaction"),
                                    tx_id.clone(),
                                    amount,
                                    price
                                )
                            }
                        }
                    }
                    Err(error) => {
                        error!("[{}]: Error while executing transaction: {:?}", exchange.get_identifier(), error);
                    }
                }
           }
        });
    }

    pub async fn run(&mut self) {
        info!("Started.");

        let tick_time = Duration::from_millis(1000); // Tick 1 times per 2 seconds.
        let mut next_tick = Instant::now();
        let mut cycles: i32 = 0;

        loop {
            let dbg = cycles % 6 == 0;
            let actionable = cycles % 20 == 0;

            self.tick_exchanges(dbg, actionable);

            while next_tick < Instant::now() {
                next_tick += tick_time;
            }

            cycles += 1;

            tokio::time::sleep_until(next_tick).await;
        }
    }

    fn tick_exchanges(&self, debug: bool, actionable: bool) {
        for (_, exchange) in self.exchanges.iter() {
            let exchange = Arc::clone(exchange);
            let debug = debug.clone();
            let actionable = actionable.clone();

            tokio::spawn(async move {
                let mut lock = exchange.lock().await;
                lock.tick(debug, actionable).await;
            });
        }
    }

    fn record_transaction_to_database<T: Into<String>>(tx_symbol: T, exchange_id: T, buy_id: T, amount: f64, price: f64) {
        use crate::schema::transactions;

        let connection = crate::DATABASE.get_connection();
        let transaction = Transaction {
            id: uuid::Uuid::new_v4().to_string(),
            exchange_name: exchange_id.into(),
            buy_exchange_id: Some(buy_id.into()),
            sell_exchange_id: None,
            amount,
            symbol: tx_symbol.into(),
            price,
            stage: TransactionStage::BuyTransactionOpen.to_string(),
            created_at: Some(Utc::now().naive_utc()),
            updated_at: Some(Utc::now().naive_utc()),
        };

        diesel::insert_into(transactions::table)
            .values(&transaction)
            .execute(&connection)
            .expect("Error saving transaction");

        info!("Inserted transaction with id {} into database", &transaction.id);
    }

    fn update_transaction_for_sale<T: Into<String>>(transaction_id: T, sell_id: T, amount: f64, price: f64) {
        use crate::schema::transactions::dsl;

        let connection = crate::DATABASE.get_connection();
        let transaction_id = transaction_id.into();
        let sell_id = sell_id.into();
        let transaction: Transaction = transactions
            .find(&transaction_id)
            .first(&connection)
            .expect("Could not find transaction");

        let change_set = UpdateTransactionStageForm {
            stage: TransactionStage::SellTransactionOpen.to_string(),
            sell_exchange_id: Some(sell_id.clone()),
            updated_at: Some(Utc::now().naive_utc()),
            amount: transaction.amount
        };

        diesel::update(&transaction)
            .set(change_set)
            .execute(&connection);

        let finished = FinishedTransaction {
            id: Uuid::new_v4().to_string(),
            transaction_id: sell_id,
            amount_bought: transaction.amount.clone(),
            buy_price: transaction.price.clone(),
            amount_sold: amount,
            sell_price: price,
            created_at: Some(Utc::now().naive_utc()),
            updated_at: Some(Utc::now().naive_utc()),
        };

        {
            use crate::schema::finished_transactions;


            diesel::insert_into(finished_transactions::table)
                .values(&finished)
                .execute(&connection)
                .expect("Error saving transaction");
        }

        info!("[Mandala]: Success!! Bought {} {} at {}. Sold {} {} at {}. making a profit of {:.2} {}.",
              &transaction.amount,
              &transaction.symbol,
              &transaction.price,
              &finished.amount_sold,
              &transaction.symbol,
              &finished.sell_price,
              (finished.amount_sold * finished.sell_price) - (transaction.amount * transaction.price),
              Config.quote_currency.clone()
        );
    }
}

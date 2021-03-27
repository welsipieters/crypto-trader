use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug, Serialize, PartialEq)]
pub struct Coin {
    pub symbol: String,
}

impl Coin {
    pub fn new<T: Into<String>>(symbol: T) -> Self {
        Self {
            symbol: symbol.into(),
        }
    }
}

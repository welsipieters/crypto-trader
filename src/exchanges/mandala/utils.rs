use crate::utils::f64_from_string;
use crate::utils::bool_from_int;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::BTreeMap;
use round::round_down;
use crate::CONFIG as Config;

#[derive(Debug, Deserialize)]
pub struct MandalaResponse<D> {
    pub code: i32,
    pub msg: String,
    pub data: D,
    pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListedResponse<I> {
    pub list: Vec<I>,
}

#[derive(Debug, Deserialize)]
pub struct Symbol {
    #[serde(rename = "type")]
    pub symbol_type: i8,
    pub symbol: String,
    #[serde(rename = "baseAsset")]
    pub base_currency: String,
    #[serde(rename = "quoteAsset")]
    pub quote_currency: String,
}

#[derive(Debug, Serialize)]
pub struct WebsocketRequest {
    method: String,
    params: Vec<String>,
    id: i8,
}

impl WebsocketRequest {
    pub fn new<T: Into<String>>(id: i8, method: T, params: Vec<String>) -> Self {
        Self {
            method: method.into(),
            params,
            id,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DepthUpdate {
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_id: i64,
    #[serde(rename = "u")]
    pub last_id: i64,
    #[serde(rename = "b")]
    pub bids: Vec<Order>,
    #[serde(rename = "a")]
    pub asks: Vec<Order>,
}

#[derive(Debug, Deserialize)]
pub struct DepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: i64,
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}

#[derive(Debug, Deserialize, Clone)]
// PRICE, QUANTITY
pub struct Order(
    #[serde(deserialize_with = "f64_from_string")] pub f64,
    #[serde(deserialize_with = "f64_from_string")] pub f64,
);

#[derive(Deserialize, Debug)]
pub struct RequestedOrder {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "clientId")]
    pub client_id: String,
    pub side: OrderSide,
    pub symbol: String,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub status: OrderStatus,
    #[serde(rename = "origQty")]
    #[serde(deserialize_with = "f64_from_string")]
    pub original_quantity: f64,
    #[serde(rename = "origQuoteQty")]
    #[serde(deserialize_with = "f64_from_string")]
    pub original_quote_quantity: f64,
    #[serde(rename = "executedQty")]
    #[serde(deserialize_with = "f64_from_string")]
    pub executed_quantity: f64,
    #[serde(rename = "executedPrice")]
    #[serde(deserialize_with = "f64_from_string")]
    pub executed_price: f64,
    #[serde(rename = "executedQuoteQty")]
    #[serde(deserialize_with = "f64_from_string")]
    pub executed_quote_quantity: f64,
    #[serde(rename = "createTime")]
    pub create_time: i64,
}

#[derive(Debug, Serialize_repr, Deserialize_repr, Copy, Clone, PartialOrd, PartialEq)]
#[repr(u8)]
pub enum OrderSide {
    Buy = 0,
    Sell = 1,
}

#[derive(Debug, Serialize_repr, Deserialize_repr, Copy, Clone, PartialOrd, PartialEq)]
#[repr(u8)]
pub enum OrderType {
    Limit = 1,
    Market = 2,
    StopLoss = 3,
    StopLossLimit = 4,
    TakeProfit = 5,
    TakeProfitLimit = 6,
    LimitMake = 7,
}

#[derive(Debug, Serialize_repr, Deserialize_repr, Copy, Clone, PartialOrd, PartialEq)]
#[repr(u8)]
pub enum OrderStatus {
    New = 0,
    PartiallyFilled = 1,
    Filled = 2,
    Canceled = 3,
    PendingCancel = 4,
    Rejected = 5,
    Expired = 6,
}

#[derive(Deserialize, Debug)]
pub struct PlaceOrderResponse {
    #[serde(rename = "orderId")]
    pub order_id: i64,
    #[serde(rename = "createTime")]
    pub create_time: i64
}

#[derive(Debug, Serialize)]
pub struct OrderRequest {
    symbol: String,
    side: OrderSide,
    order_type: OrderType,
    quantity: Option<f64>,
    quote_order_quantity: Option<f64>,
    price: Option<f64>,
    stop_price: Option<f64>,
    client_id: Option<String>,
    iceberg_qty: Option<f64>
}

impl OrderRequest {
    pub fn new(symbol: String, side: OrderSide, quantity: Option<f64>, price: Option<f64>) -> Self {
        assert_eq!(symbol.contains(format!("_{}", &Config.quote_currency).as_str()), true, "Symbol must be in BASE_QUOTE format");

        Self {
            symbol,
            side,
            order_type: OrderType::Limit,
            quantity,
            quote_order_quantity: None,
            price,
            stop_price: None,
            client_id: None,
            iceberg_qty: None
        }
    }
}


impl OrderRequest {
    pub fn to_map(self) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        map.insert("symbol".into(), self.symbol);
        map.insert("side".into(), format!("{}", self.side as u8));
        map.insert("type".into(), format!("{}", self.order_type as u8));

        if let Some(quantity) = self.quantity {
            map.insert("quantity".into(), format!("{}", round_down(quantity, 1)));
        }

        if let Some(quote_order_quantity) = self.quote_order_quantity {
            map.insert("quote_order_quantity".into(), format!("{}", quote_order_quantity));
        }

        if let Some(price) = self.price {
            map.insert("price".into(), format!("{}", price));
        }

        if let Some(stop_price) = self.stop_price {
            map.insert("stop_price".into(), format!("{}", stop_price));
        }

        if let Some(client_id) = self.client_id.clone() {
            map.insert("client_id".into(), format!("{}", client_id));
        }

        if let Some(iceberg_qty) = self.iceberg_qty {
            map.insert("iceberg_qty".into(), format!("{}", iceberg_qty));
        }

        map
    }
}

#[derive(Deserialize, Debug)]
pub struct AccountInfo {
    #[serde(rename = "makerCommission")]
    #[serde(deserialize_with = "f64_from_string")]
    pub maker_commission: f64,

    #[serde(rename = "takerCommission")]
    #[serde(deserialize_with = "f64_from_string")]
    pub taker_commission: f64,

    #[serde(rename = "buyerCommission")]
    #[serde(deserialize_with = "f64_from_string")]
    pub buyer_commission: f64,

    #[serde(rename = "sellerCommission")]
    #[serde(deserialize_with = "f64_from_string")]
    pub seller_commission: f64,

    #[serde(rename = "canTrade")]
    #[serde(deserialize_with = "bool_from_int")]
    pub can_trade: bool,

    #[serde(rename = "canWithdraw")]
    #[serde(deserialize_with = "bool_from_int")]
    pub can_withdraw: bool,

    #[serde(rename = "canDeposit")]
    #[serde(deserialize_with = "bool_from_int")]
    pub can_deposit: bool,

    #[serde(rename = "accountAssets")]
    pub account_assets: Vec<AccountAsset>
}

#[derive(Deserialize, Debug)]
pub struct AccountAsset {
    pub asset: String,
    #[serde(deserialize_with = "f64_from_string")]
    pub free: f64,
    #[serde(deserialize_with = "f64_from_string")]
    pub locked: f64
}
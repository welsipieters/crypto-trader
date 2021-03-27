use hashbrown::HashMap;

pub struct BalanceMap {
    inner: HashMap<String, Balance>,
}

impl BalanceMap {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn reload(&mut self, inner: HashMap<String, Balance>) {
        self.inner = inner;
    }

    pub fn get_balance_for_symbol<T: Into<String>>(&self, symbol: T) -> Option<&Balance> {
        self.inner.get(&symbol.into())
    }
}

#[derive(Debug, Clone)]
pub struct Balance {
    pub symbol: String,
    pub available: f64,
    pub locked: f64,
}

impl Balance {
    pub fn new<T: Into<String>>(symbol: T, available: f64, locked: f64) -> Self {
        Self {
            symbol: symbol.into(),
            available,
            locked,
        }
    }
}

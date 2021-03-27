use decorum::Finite;
use hashbrown::HashMap;
use itertools::Itertools;
use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
};
use uuid::Uuid;

// Disallows NaN and Infinite
// Implements Ord
type Fin64 = Finite<f64>;

#[derive(Debug)]
pub struct OrderBook {
    symbol: String,
    pub asks: Ledger,
    pub bids: Ledger,
}

impl OrderBook {
    pub fn new<T: Into<String>>(symbol: T) -> Self {
        Self {
            symbol: symbol.into(),
            asks: Ledger::new(TailOrdering::Lowest),
            bids: Ledger::new(TailOrdering::Highest),
        }
    }

    #[inline]
    pub fn lowest_ask(&self) -> Option<Fin64> {
        self.asks.tail()
    }

    #[inline]
    pub fn highest_bid(&self) -> Option<Fin64> {
        self.bids.tail()
    }

    pub fn spread(&self) -> Option<f64> {
        if let (Some(bid), Some(ask)) = (self.highest_bid(), self.lowest_ask()) {
            return Some((bid - ask).into());
        }

        None
    }

    pub fn execute(&mut self, order: &Order) {
        match order.side {
            OrderSide::Buy => &mut self.bids,

            OrderSide::Sell => &mut self.asks,
        }
        .put(
            order.price.into(),
            if order.quantity == 0.0 {
                None
            } else {
                Some(order.quantity)
            },
        )
    }

    pub fn reload(&mut self, bids: Vec<Order>, asks: Vec<Order>) {
        self.bids.clear();
        self.asks.clear();

        self.update(bids, asks);
    }

    pub fn update(&mut self, bids: Vec<Order>, asks: Vec<Order>) {
        bids.iter().for_each(|order| {
            self.execute(order);
        });

        asks.iter().for_each(|order| {
            self.execute(order);
        });
    }

    pub fn print_self(&self) {
        println!("{}", self);
    }
}

impl Display for OrderBook {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.highest_bid().is_none() || self.lowest_ask().is_none() {
            return write!(f, "OB: empty");
        }

        let size = 10;
        let round_lambda = |x: f64| (x * 10000.0).round() / 10000.0;

        let bids = "bids";
        let asks = "asks";

        write!(
            f,
            "OB: {} | {:.3}   {:.3} | {}",
            bids,
            self.highest_bid().unwrap(),
            self.lowest_ask().unwrap(),
            asks
        )
    }
}

#[derive(Debug)]
pub enum TailOrdering {
    Lowest,
    Highest,
}

impl TailOrdering {
    fn cmp<T: Ord>(&self, a: &T, b: &T) -> Ordering {
        let ordering = a.cmp(b);

        if let TailOrdering::Lowest = self {
            // with Lowest, ordering has to be reversed, because we want the lowest value to be the "last", the "greatest"
            // we check a for new tail with new > old, if new is "greater" (Ordering::Greater), then we replace it.
            ordering.reverse()
        } else {
            ordering
        }
    }
}

#[derive(Debug)]
pub enum TailUpdate {
    Set(Fin64),
    Remove(Fin64),
}

#[derive(Debug)]
pub struct Ledger {
    map: HashMap<Fin64, f64>,
    tail: Option<Fin64>,
    ordering: TailOrdering,
}

impl Ledger {
    pub fn new(ordering: TailOrdering) -> Ledger {
        Ledger {
            map: HashMap::new(),
            tail: None,
            ordering,
        }
    }

    // quantity -> Some: updates/inserts
    // quantity -> None: removes
    // note: 0 for quantity will *insert*, not remove
    pub fn put(&mut self, price: Fin64, quantity: Option<f64>) {
        match quantity {
            Some(q) => {
                self.set(price, q);
                self.maybe_update_tail(TailUpdate::Set(price));
            }
            None => {
                // Prevent double-deletes
                if self.remove(price) {
                    self.maybe_update_tail(TailUpdate::Remove(price))
                }
            }
        }
    }

    pub fn maybe_update_tail(&mut self, update: TailUpdate) {
        match update {
            // A new value has been set
            TailUpdate::Set(at) => {
                if {
                    // basically: "should we update/replace the tail with the new value?"
                    match self.tail {
                        // "yes, there exists no old value"
                        None => true,
                        // "maybe, if the new tail is 'greater' than the old one"
                        Some(old) => self.ordering.cmp(&at, &old) == Ordering::Greater,
                    }
                } {
                    self.tail = Some(at)
                }
            }
            // A value has been removed
            TailUpdate::Remove(at) => {
                // We only care if the current tail has a value in it.
                // (Not that it is expected a "value remove" happens when the ledger is empty, but still)
                if let Some(old) = self.tail {
                    // How does the new value compare to the old tail?
                    match self.ordering.cmp(&old, &at) {
                        // If the tail is the same value of the value being removed, we must clear it and find a new value.
                        Ordering::Equal => {
                            // Replace the tail, find_new_tail() will be None if no value exists in the map anymore,
                            // which is fine, because then the tail doesn't exist.
                            self.tail = self.find_new_tail()
                        }

                        // If the current tail is somehow(???) "less" than the removed value,
                        // we must panic, because this *should never happen*.
                        //
                        // *If* this ever happens, it denotes a tail not being calculated correctly somewhere,
                        // because this path (TailUpdate::Remove) is the only one that CAN cut off the tail to "lesser"
                        // value, and because EVERY removal is handled by ::Equal, this path should *never* trigger,
                        // because that means that an ::Equal call was missed, the hashmap was edited without methods of
                        // Ledger, or something else is fucking up.
                        Ordering::Less => {
                            panic!("Found Ordering::Less for TailOrdering {:?} for values (a) {} and (b) {}", self.ordering, old, at)
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn find_new_tail(&self) -> Option<Fin64> {
        let keys = self.map.keys();
        match self.ordering {
            TailOrdering::Lowest => {
                // We want the lowest value for the tail
                keys.min()
            }
            TailOrdering::Highest => {
                // We want the highest value for the tail
                keys.max()
            }
        }
        .cloned()
    }

    // doesn't allow mut
    #[inline]
    pub fn tail(&self) -> Option<Fin64> {
        self.tail
    }

    #[inline]
    fn set<F: Into<Fin64>>(&mut self, at: F, with: f64) {
        self.map.insert(at.into(), with);
    }

    #[inline]
    fn remove<F: Into<Fin64>>(&mut self, at: F) -> bool {
        self.map.remove(&at.into()).is_some()
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.tail = None;
    }

    pub fn iter(&self) -> std::vec::IntoIter<(&Fin64, &f64)> {
        self.map
            .iter()
            .sorted_by(|a, b| self.ordering.cmp(&b.0, &a.0))
    }
}

#[derive(Debug)]
pub struct Order {
    // Food for thought:
    // If we get local orderbooks working really well we might be able to actually match orders
    // to our local book instead of just relying on the data sent by an exchange. That could
    // potentially make trading faster and smarter?
    side: OrderSide,
    quantity: f64,
    price: f64,
}

impl Order {
    pub fn new(side: OrderSide, quantity: f64, price: f64) -> Self {
        Self {
            side,
            quantity,
            price,
        }
    }
}

#[derive(Debug)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug)]
pub enum OrderType {
    Limit,
    Market,
}


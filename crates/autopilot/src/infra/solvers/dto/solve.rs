use {
    crate::{
        boundary,
        domain::{self, eth},
        infra::persistence::dto::{self, order::Order},
    },
    alloy::primitives::{Address, U256},
    chrono::{DateTime, Utc},
    itertools::Itertools,
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
        time::Duration,
    },
};

/// Cheaply clonable handle to an already JSON serialized
/// request. The purpose of this is to make it ergonomic
/// to serialize a request once and reuse the resulting
/// string in multiple HTTP requests.
#[derive(Clone, Debug, Serialize, derive_more::Display)]
pub struct Request(Arc<serde_json::value::RawValue>);

impl Request {
    pub fn as_str(&self) -> &str {
        self.0.get()
    }
}

impl Request {
    pub fn new(
        auction: &domain::Auction,
        trusted_tokens: &HashSet<Address>,
        time_limit: Duration,
    ) -> Self {
        let _timer =
            observe::metrics::metrics().on_auction_overhead_start("autopilot", "serialize_request");
        let helper = RequestHelper {
            id: auction.id,
            orders: auction
                .orders
                .clone()
                .into_iter()
                .map(dto::order::from_domain)
                .collect(),
            tokens: auction
                .prices
                .iter()
                .map(|(address, price)| Token {
                    address: address.to_owned().0,
                    price: Some(price.get().0),
                    trusted: trusted_tokens.contains(&(address.0)),
                })
                .chain(trusted_tokens.iter().map(|&address| Token {
                    address,
                    price: None,
                    trusted: true,
                }))
                .unique_by(|token| token.address)
                .collect(),
            deadline: Utc::now() + chrono::Duration::from_std(time_limit).unwrap(),
            surplus_capturing_jit_order_owners: auction.surplus_capturing_jit_order_owners.to_vec(),
        };
        Self(Arc::from(serde_json::value::to_raw_value(&helper).expect(
            "only fails with non-string keys which we do not have",
        )))
    }
}

impl Response {
    pub fn into_domain(
        self,
    ) -> Vec<Result<domain::competition::Solution, domain::competition::SolutionError>> {
        self.solutions
            .into_iter()
            .map(Solution::into_domain)
            .collect()
    }
}

#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestHelper {
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    pub tokens: Vec<Token>,
    pub orders: Vec<Order>,
    pub deadline: DateTime<Utc>,
    pub surplus_capturing_jit_order_owners: Vec<Address>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub address: Address,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub price: Option<U256>,
    pub trusted: bool,
}

impl Solution {
    pub fn into_domain(
        self,
    ) -> Result<domain::competition::Solution, domain::competition::SolutionError> {
        Ok(domain::competition::Solution::new(
            self.solution_id,
            self.submission_address,
            self.orders
                .into_iter()
                .map(|(o, amounts)| (o.into(), amounts.into_domain()))
                .collect(),
            self.clearing_prices
                .into_iter()
                .map(|(token, price)| {
                    domain::auction::Price::try_new(price.into()).map(|price| (token.into(), price))
                })
                .collect::<Result<_, _>>()?,
        ))
    }
}

/// Contains basic order information and the executed amounts. Basic order
/// information are required because of JIT orders which are not part of an
/// auction, so autopilot can be aware of them before the solution is
/// settled on-chain.
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradedOrder {
    side: Side,
    sell_token: Address,
    buy_token: Address,
    /// Sell limit order amount.
    #[serde_as(as = "HexOrDecimalU256")]
    limit_sell: U256,
    /// Buy limit order amount.
    #[serde_as(as = "HexOrDecimalU256")]
    limit_buy: U256,
    /// The effective amount that left the user's wallet including all fees.
    #[serde_as(as = "HexOrDecimalU256")]
    executed_sell: U256,
    /// The effective amount the user received after all fees.
    #[serde_as(as = "HexOrDecimalU256")]
    executed_buy: U256,
}

impl TradedOrder {
    pub fn into_domain(self) -> domain::competition::TradedOrder {
        domain::competition::TradedOrder {
            sell: eth::Asset {
                token: self.sell_token.into(),
                amount: self.limit_sell.into(),
            },
            buy: eth::Asset {
                token: self.buy_token.into(),
                amount: self.limit_buy.into(),
            },
            side: match self.side {
                Side::Buy => domain::auction::order::Side::Buy,
                Side::Sell => domain::auction::order::Side::Sell,
            },
            executed_sell: self.executed_sell.into(),
            executed_buy: self.executed_buy.into(),
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Unique ID of the solution (per driver competition), used to identify
    /// it in subsequent requests (reveal, settle).
    pub solution_id: u64,
    /// Address used by the driver to submit the settlement onchain.
    pub submission_address: Address,
    pub orders: HashMap<boundary::OrderUid, TradedOrder>,
    #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
    pub clearing_prices: HashMap<Address, U256>,
    pub gas: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub solutions: Vec<Solution>,
}

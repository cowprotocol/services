use {
    crate::{
        boundary,
        domain::{self, eth},
        infra::persistence::{dto, dto::order::Order},
    },
    chrono::{DateTime, Utc},
    itertools::Itertools,
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    std::{
        collections::{HashMap, HashSet},
        time::Duration,
    },
};

impl Request {
    pub fn new(
        auction: &domain::Auction,
        trusted_tokens: &HashSet<H160>,
        time_limit: Duration,
    ) -> Self {
        Self {
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
                    address: address.to_owned().into(),
                    price: Some(price.get().into()),
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
            surplus_capturing_jit_order_owners: auction
                .surplus_capturing_jit_order_owners
                .iter()
                .map(|address| address.0)
                .collect::<Vec<_>>(),
        }
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
pub struct Request {
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    pub tokens: Vec<Token>,
    pub orders: Vec<Order>,
    pub deadline: DateTime<Utc>,
    pub surplus_capturing_jit_order_owners: Vec<H160>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub address: H160,
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
            self.submission_address.into(),
            domain::competition::Score::try_new(self.score.into())?,
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
    sell_token: H160,
    buy_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    /// Sell limit order amount.
    limit_sell: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    /// Buy limit order amount.
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
    #[serde_as(as = "HexOrDecimalU256")]
    pub score: U256,
    /// Address used by the driver to submit the settlement onchain.
    pub submission_address: H160,
    pub orders: HashMap<boundary::OrderUid, TradedOrder>,
    #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
    pub clearing_prices: HashMap<H160, U256>,
    pub gas: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub solutions: Vec<Solution>,
}

use {
    crate::{
        boundary,
        domain,
        infra::persistence::{dto, dto::order::Order},
    },
    chrono::{DateTime, Utc},
    itertools::Itertools,
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
        id: domain::AuctionId,
        auction: &domain::Auction,
        trusted_tokens: &HashSet<H160>,
        score_cap: U256,
        time_limit: Duration,
    ) -> Self {
        Self {
            id,
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
                    address: address.to_owned(),
                    price: Some(price.to_owned().into()),
                    trusted: trusted_tokens.contains(address),
                })
                .chain(trusted_tokens.iter().map(|&address| Token {
                    address,
                    price: None,
                    trusted: true,
                }))
                .unique_by(|token| token.address)
                .collect(),
            deadline: Utc::now() + chrono::Duration::from_std(time_limit).unwrap(),
            score_cap: score_cap.into(),
        }
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
    pub score_cap: number::U256,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub address: H160,
    pub price: Option<number::U256>,
    pub trusted: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TradedAmounts {
    /// The effective amount that left the user's wallet including all fees.
    pub sell_amount: number::U256,
    /// The effective amount the user received after all fees.
    pub buy_amount: number::U256,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Solution {
    /// Unique ID of the solution (per driver competition), used to identify
    /// it in subsequent requests (reveal, settle).
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub solution_id: u64,
    pub score: number::U256,
    /// Address used by the driver to submit the settlement onchain.
    pub submission_address: H160,
    pub orders: HashMap<boundary::OrderUid, TradedAmounts>,
    pub clearing_prices: HashMap<H160, number::U256>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    pub solutions: Vec<Solution>,
}

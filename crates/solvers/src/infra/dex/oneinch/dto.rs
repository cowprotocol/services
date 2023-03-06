//! DTOs for the 1Inch DEX aggregator API. Full documentation for the API can be
//! found [here](https://docs.1inch.io/docs/aggregation-protocol/api/swagger).

use {
    crate::{
        domain::{auction, dex, order},
        util::serialize,
    },
    bigdecimal::BigDecimal,
    ethereum_types::{H160, U256},
    num::BigInt,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// The allowance spender data for 1Inch swaps.
#[derive(Deserialize)]
pub struct Spender {
    pub address: H160,
}

/// A protocol that is supported by the 1Inch API.
#[derive(Deserialize)]
pub struct Protocol {
    pub id: String,
}

/// A collection of supported liquidity sources by the 1Inch API.
#[derive(Deserialize)]
pub struct Liquidity {
    pub protocols: Vec<Protocol>,
}

/// A 1Inch API swap query parameters.
///
/// See [API](https://docs.1inch.io/docs/aggregation-protocol/api/swagger)
/// documentation for more details.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    /// Contract address of a token to sell.
    pub from_token_address: H160,

    /// Contract address of a token to buy.
    pub to_token_address: H160,

    /// Amount of a token to sell, set in atoms.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,

    /// The address that calls the 1Inch contract to execute the returned swap.
    pub from_address: H160,

    /// The maximum negative slippage allowed for swapping.
    pub slippage: Slippage,

    /// List of sources to exclude.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::CommaSeparated>")]
    pub protocols: Option<Vec<String>>,

    /// The referrer address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer_address: Option<H160>,

    /// Disable 1Inch swap estimates. Normally, the 1Inch API will simulate and
    /// verify the swap. However, this requires upfront balances and approvals
    /// which are not always available (and in the case of the CoW Protocol
    /// settlement contract, usually not available). This flag can be set in
    /// order to disable the simulation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_estimate: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_route_parts: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity_level: Option<u32>,

    /// The target gas price for the swap transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub gas_price: Option<U256>,
}

impl Query {
    pub fn with_domain(
        self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        gas_price: auction::GasPrice,
    ) -> Option<Self> {
        // Buy orders are not supported on 1Inch
        if order.side == order::Side::Buy {
            return None;
        };

        Some(Self {
            from_token_address: order.sell.0,
            to_token_address: order.buy.0,
            amount: order.amount.get(),
            slippage: Slippage::from_domain(slippage),
            gas_price: Some(gas_price.0 .0),
            ..self
        })
    }
}

/// A 1Inch slippage amount.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Slippage(BigDecimal);

impl Slippage {
    /// Returns a 1Inch slippage amount.
    fn from_domain(slippage: &dex::Slippage) -> Self {
        // 1Inch API expects slippage to be a percentage only accepts up to 4
        // digits of precision.
        // <https://github.com/cowprotocol/services/pull/585>
        // <https://github.com/cowprotocol/services/pull/589>
        // <https://github.com/cowprotocol/services/pull/600>
        Self((slippage.round(6).as_factor() * BigInt::from(100)).normalized())
    }
}

/// A 1Inch API swap response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    /// Amount of source token.
    #[serde_as(as = "serialize::U256")]
    pub from_token_amount: U256,

    /// Expected amount of destination token.
    #[serde_as(as = "serialize::U256")]
    pub to_token_amount: U256,

    /// The corresponding transaction for the swap.
    pub tx: Tx,
}

/// 1Inch swap transaction data.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tx {
    /// Amount of source token.
    pub to: H160,

    /// Expected amount of destination token.
    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response {
    Ok(Swap),
    Err(Error),
}

impl Response {
    /// Turns the API response into a [`std::result::Result`].
    pub fn into_result(self) -> Result<Swap, Error> {
        match self {
            Response::Ok(swap) => Ok(swap),
            Response::Err(err) => Err(err),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub status_code: i32,
    pub description: String,
}

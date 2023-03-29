//! DTOs for the 0x swap API. Full documentation for the API can be found
//! [here](https://docs.0x.org/0x-api-swap/api-references/get-swap-v1-quote).

use {
    crate::{
        domain::{auction, dex, order},
        util::serialize,
    },
    bigdecimal::BigDecimal,
    ethereum_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// A 0x API quote query parameters.
///
/// See [API](https://docs.0x.org/0x-api-swap/api-references/get-swap-v1-quote)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    /// Contract address of a token to sell.
    pub sell_token: H160,

    /// Contract address of a token to buy.
    pub buy_token: H160,

    /// Amount of a token to sell, set in atoms.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub sell_amount: Option<U256>,

    /// Amount of a token to sell, set in atoms.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub buy_amount: Option<U256>,

    /// Limit of price slippage you are willing to accept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_percentage: Option<Slippage>,

    /// The target gas price for the swap transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub gas_price: Option<U256>,

    /// The address which will fill the quote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taker_address: Option<H160>,

    /// List of sources to exclude.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "serialize::CommaSeparated")]
    pub excluded_sources: Vec<String>,

    /// Whether or not to skip quote validation.
    pub skip_validation: bool,

    /// Wether or not you intend to actually fill the quote. Setting this flag
    /// enables RFQ-T liquidity.
    ///
    /// <https://docs.0x.org/market-makers/docs/introduction>
    pub intent_on_filling: bool,

    /// The affiliate address to use for tracking and analytics purposes.
    pub affiliate_address: H160,

    /// Requests trade routes which aim to protect against high slippage and MEV
    /// attacks.
    pub enable_slippage_protection: bool,
}

/// A 0x slippage amount.
#[derive(Clone, Debug, Serialize)]
pub struct Slippage(BigDecimal);

impl Query {
    pub fn with_domain(
        self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        gas_price: auction::GasPrice,
    ) -> Self {
        let (sell_amount, buy_amount) = match order.side {
            order::Side::Buy => (None, Some(order.amount.get())),
            order::Side::Sell => (Some(order.amount.get()), None),
        };

        Self {
            sell_token: order.sell.0,
            buy_token: order.buy.0,
            sell_amount,
            buy_amount,
            // Note that the API calls this "slippagePercentage", but it is **not** a
            // percentage but a factor.
            slippage_percentage: Some(Slippage(slippage.as_factor().clone())),
            gas_price: Some(gas_price.0 .0),
            ..self
        }
    }
}

/// A Ox API quote response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// The address of the contract to call in order to execute the swap.
    pub to: H160,

    /// The swap calldata.
    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,

    /// The estimate for the amount of gas that will actually be used in the
    /// transaction.
    #[serde_as(as = "serialize::U256")]
    pub estimated_gas: U256,

    /// The amount of sell token (in atoms) that would be sold in this swap.
    #[serde_as(as = "serialize::U256")]
    pub sell_amount: U256,

    /// The amount of buy token (in atoms) that would be bought in this swap.
    #[serde_as(as = "serialize::U256")]
    pub buy_amount: U256,

    /// The target contract address for which the user needs to have an
    /// allowance in order to be able to complete the swap.
    #[serde(with = "address_none_when_zero")]
    pub allowance_target: Option<H160>,
}

/// The 0x API uses the 0-address to indicate that no approvals are needed for a
/// swap. Use a custom deserializer to turn that into `None`.
mod address_none_when_zero {
    use {
        ethereum_types::H160,
        serde::{Deserialize, Deserializer},
    };

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<H160>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = H160::deserialize(deserializer)?;
        Ok((!value.is_zero()).then_some(value))
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response {
    Ok(Quote),
    Err(Error),
}

impl Response {
    /// Turns the API response into a [`std::result::Result`].
    pub fn into_result(self) -> Result<Quote, Error> {
        match self {
            Response::Ok(quote) => Ok(quote),
            Response::Err(err) => Err(err),
        }
    }
}

#[derive(Deserialize)]
pub struct Error {
    pub code: i64,
    pub reason: String,
}

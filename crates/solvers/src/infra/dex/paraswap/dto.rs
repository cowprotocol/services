//! DTOs for the ParaSwap swap API. Full documentation for the API can be found
//! [here](https://developers.paraswap.network/api/get-rate-for-a-token-pair).

use {
    crate::{
        domain::{auction, dex, order},
        util::serialize,
    },
    ethereum_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// ParaSwap query parameters for the `/prices` endpoint.
///
/// See [API](https://developers.paraswap.network/api/get-rate-for-a-token-pair)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceQuery {
    /// Source token address.
    pub src_token: H160,

    /// Destination token address.
    pub dest_token: H160,

    /// Source token decimals.
    pub src_decimals: u8,

    /// Destination token decimals.
    pub dest_decimals: u8,

    /// Source token amount when the side is "sell" or destination token amount
    /// when the side is "buy". The amount should be in atoms.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,

    /// Sell or buy?
    pub side: Side,

    /// The list of DEXs to exclude from the computed price route.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "serialize::CommaSeparated")]
    pub exclude_dexs: Vec<String>,

    /// The network ID.
    pub network: String,
}

impl PriceQuery {
    pub fn new(
        config: &super::Config,
        order: &dex::Order,
        tokens: &auction::Tokens,
    ) -> Result<Self, super::Error> {
        Ok(Self {
            src_token: order.sell.0,
            dest_token: order.buy.0,
            src_decimals: tokens
                .decimals(&order.sell)
                .ok_or(super::Error::MissingDecimals)?,
            dest_decimals: tokens
                .decimals(&order.buy)
                .ok_or(super::Error::MissingDecimals)?,
            side: match order.side {
                order::Side::Buy => Side::Buy,
                order::Side::Sell => Side::Sell,
            },
            amount: order.amount.get(),
            exclude_dexs: config.exclude_dexs.clone(),
            network: "1".to_owned(),
        })
    }
}

/// ParaSwap API body parameters for the `/transactions` endpoint.
///
/// See [API](https://developers.paraswap.network/api/build-parameters-for-transaction)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionBody {
    /// Source token address.
    pub src_token: H160,

    /// Destination token address.
    pub dest_token: H160,

    // Source amount.
    #[serde_as(as = "serialize::U256")]
    pub src_amount: U256,

    // Destination amount.
    #[serde_as(as = "serialize::U256")]
    pub dest_amount: U256,

    /// The decimals of the source token.
    pub src_decimals: u8,

    /// The decimals of the destination token.
    pub dest_decimals: u8,

    /// Price route from `/prices` endpoint response (without any change).
    pub price_route: serde_json::Value,

    /// The address of the signer.
    pub user_address: H160,

    /// The partner name.
    pub partner: Option<String>,
}

impl TransactionBody {
    pub fn new(
        price: &Price,
        config: &super::Config,
        order: &dex::Order,
        tokens: &auction::Tokens,
        slippage: &dex::Slippage,
    ) -> Result<Self, super::Error> {
        let (src_amount, dest_amount) = match order.side {
            order::Side::Sell => (price.src_amount()?, slippage.sub(price.dest_amount()?)),
            order::Side::Buy => (slippage.add(price.src_amount()?), price.dest_amount()?),
        };
        Ok(Self {
            src_token: order.sell.0,
            dest_token: order.buy.0,
            src_decimals: tokens
                .decimals(&order.sell)
                .ok_or(super::Error::MissingDecimals)?,
            dest_decimals: tokens
                .decimals(&order.buy)
                .ok_or(super::Error::MissingDecimals)?,
            src_amount,
            dest_amount,
            price_route: price.price_route.clone(),
            user_address: config.address,
            partner: config.partner.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Sell,
    Buy,
}

/// A ParaSwap API price response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    /// The price route. This should be passed on to the `/transactions`
    /// endpoint.
    pub price_route: serde_json::Value,
}

impl Price {
    pub fn src_amount(&self) -> Result<U256, serde_json::Error> {
        serde_json::from_value::<PriceRoute>(self.price_route.clone()).map(|r| r.src_amount)
    }

    pub fn dest_amount(&self) -> Result<U256, serde_json::Error> {
        serde_json::from_value::<PriceRoute>(self.price_route.clone()).map(|r| r.dest_amount)
    }

    pub fn gas_cost(&self) -> Result<U256, serde_json::Error> {
        serde_json::from_value::<PriceRoute>(self.price_route.clone()).map(|r| r.gas_cost)
    }

    pub fn token_transfer_proxy(&self) -> Result<H160, serde_json::Error> {
        serde_json::from_value::<PriceRoute>(self.price_route.clone())
            .map(|r| r.token_transfer_proxy)
    }
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PriceRoute {
    #[serde_as(as = "serialize::U256")]
    src_amount: U256,
    #[serde_as(as = "serialize::U256")]
    dest_amount: U256,
    #[serde_as(as = "serialize::U256")]
    gas_cost: U256,
    token_transfer_proxy: H160,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub from: H160,
    pub to: H160,

    #[serde_as(as = "serialize::U256")]
    pub value: U256,

    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response<T> {
    Ok(T),
    Err(Error),
}

impl<T> Response<T> {
    /// Turns the API response into a [`std::result::Result`].
    pub fn into_result(self) -> Result<T, Error> {
        match self {
            Response::Ok(quote) => Ok(quote),
            Response::Err(err) => Err(err),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub error: String,
}

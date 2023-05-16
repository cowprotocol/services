//! DTOs for the ParaSwap swap API. Full documentation for the API can be found
//! [here](https://developers.paraswap.network/api/get-rate-for-a-token-pair).

use {
    crate::{
        domain::{auction::Auction, dex, order},
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
}

impl PriceQuery {
    pub fn new(config: &super::Config, order: &dex::Order, auction: &Auction) -> Self {
        Self {
            src_token: order.sell.0,
            dest_token: order.buy.0,
            src_decimals: auction
                .tokens
                .get(&order.sell)
                .unwrap()
                .decimals
                .unwrap_or(18),
            dest_decimals: auction
                .tokens
                .get(&order.buy)
                .unwrap()
                .decimals
                .unwrap_or(18),
            side: match order.side {
                order::Side::Buy => Side::Buy,
                order::Side::Sell => Side::Sell,
            },
            amount: order.amount.get(),
            exclude_dexs: config.exclude_dexs.clone(),
        }
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

    #[serde(flatten)]
    pub trade_amount: TradeAmount,

    /// The decimals of the source token.
    pub src_decimals: u8,

    /// The decimals of the destination token.
    pub dest_decimals: u8,

    /// Price route from `/prices` endpoint response (without any change).
    pub price_route: serde_json::Value,

    /// The address of the signer.
    pub user_address: H160,
}

impl TransactionBody {
    pub fn new(
        price: &Price,
        config: &super::Config,
        order: &dex::Order,
        auction: &Auction,
        slippage: &dex::Slippage,
    ) -> Self {
        Self {
            src_token: order.sell.0,
            dest_token: order.buy.0,
            src_decimals: auction
                .tokens
                .get(&order.sell)
                .unwrap()
                .decimals
                .unwrap_or(18),
            dest_decimals: auction
                .tokens
                .get(&order.buy)
                .unwrap()
                .decimals
                .unwrap_or(18),
            trade_amount: match order.side {
                order::Side::Sell => TradeAmount::Exact {
                    src_amount: price.src_amount(),
                    dest_amount: slippage.sub(price.dest_amount()),
                },
                order::Side::Buy => TradeAmount::Exact {
                    src_amount: slippage.add(price.src_amount()),
                    dest_amount: price.dest_amount(),
                },
            },
            price_route: price.price_route.clone(),
            user_address: config.address,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Sell,
    Buy,
}

/// The amounts for buying and selling.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
#[serde_as]
pub enum TradeAmount {
    /// For a sell order, specify the sell amount and slippage used for
    /// determining the minimum buy amount.
    #[serde(rename_all = "camelCase")]
    Sell {
        /// The source amount.
        #[serde_as(as = "serialize::U256")]
        src_amount: U256,
        /// The maximum slippage in the range [0, 10000].
        slippage: u32,
    },
    /// For a buy order, specify the buy amount and slippage used for
    /// determining the maximum sell amount.
    #[serde(rename_all = "camelCase")]
    Buy {
        /// The destination amount.
        #[serde_as(as = "serialize::U256")]
        dest_amount: U256,
        /// The maximum slippage in the range [0, 10000].
        slippage: u32,
    },
    // TODO It seems like I only need Exact, I can get rid of the other ones
    /// For any order (buy or sell), specify the limit amounts for building
    /// the transaction. The order "side" (i.e. buy or sell) is determined based
    /// on the initial `/price` query and the included `price_route`.
    #[serde(rename_all = "camelCase")]
    Exact {
        #[serde_as(as = "serialize::U256")]
        src_amount: U256,
        #[serde_as(as = "serialize::U256")]
        dest_amount: U256,
    },
}

/// A ParaSwap API price response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    /// The price route. This should be passed back to the `/transactions`
    /// endpoint.
    pub price_route: serde_json::Value,
}

impl Price {
    pub fn src_amount(&self) -> U256 {
        todo!()
    }

    pub fn dest_amount(&self) -> U256 {
        todo!()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde_as]
pub struct Transaction {
    pub from: H160,
    pub to: H160,

    #[serde_as(as = "serialize::U256")]
    pub value: U256,

    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,

    #[serde_as(as = "serialize::U256")]
    pub gas: U256,
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
pub struct Error {
    pub error: String,
}

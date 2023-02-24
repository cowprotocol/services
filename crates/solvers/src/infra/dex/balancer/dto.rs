use {
    crate::{
        domain::{auction, dex, order},
        util::serialize,
    },
    ethereum_types::{H160, H256, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderKind {
    Sell,
    Buy,
}

/// An SOR query.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    /// The sell token to quote.
    pub sell_token: H160,
    /// The buy token to quote.
    pub buy_token: H160,
    /// The order kind to use.
    pub order_kind: OrderKind,
    /// The amount to quote
    ///
    /// For sell orders this is the exact amount of sell token to trade, for buy
    /// orders, this is the amount of buy tokens to buy.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,
    /// The current gas price estimate used for determining how the trading
    /// route should be split.
    #[serde_as(as = "serialize::U256")]
    pub gas_price: U256,
}

impl Query {
    pub fn from_domain(order: &dex::Order, gas_price: auction::GasPrice) -> Self {
        Self {
            sell_token: order.sell.0,
            buy_token: order.buy.0,
            order_kind: match order.side {
                order::Side::Buy => OrderKind::Buy,
                order::Side::Sell => OrderKind::Sell,
            },
            amount: order.amount().amount,
            gas_price: gas_price.0 .0,
        }
    }
}

/// The swap route found by the Balancer SOR service.
#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// The token addresses included in the swap route.
    pub token_addresses: Vec<H160>,
    /// The swap route.
    pub swaps: Vec<Swap>,
    /// The swapped token amount.
    ///
    /// In sell token for sell orders or buy token for buy orders.
    #[serde_as(as = "serialize::U256")]
    pub swap_amount: U256,
    /// The real swapped amount for certain kinds of wrapped tokens.
    ///
    /// Some wrapped tokens like stETH/wstETH support wrapping and unwrapping at
    /// a conversion rate before trading using a Relayer. In those cases, this
    /// amount represents the value of the real token before wrapping.
    ///
    /// This amount is useful for informational purposes and not intended to be
    /// used when calling `singleSwap` an `batchSwap` on the Vault.
    #[serde_as(as = "serialize::U256")]
    pub swap_amount_for_swaps: U256,
    /// The returned token amount.
    ///
    /// In buy token for sell orders or sell token for buy orders.
    #[serde_as(as = "serialize::U256")]
    pub return_amount: U256,
    /// The real returned amount.
    ///
    /// See `swap_amount_for_swap` for more details.
    #[serde_as(as = "serialize::U256")]
    pub return_amount_from_swaps: U256,
    /// The received considering fees.
    ///
    /// This can be negative when quoting small sell amounts at high gas costs
    /// or greater than `U256::MAX` when quoting large buy amounts at high
    /// gas costs.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub return_amount_considering_fees: num::BigInt,
    /// The input (sell) token.
    #[serde(with = "address_default_when_empty")]
    pub token_in: H160,
    /// The output (buy) token.
    #[serde(with = "address_default_when_empty")]
    pub token_out: H160,
    /// The price impact (i.e. market slippage).
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub market_sp: f64,
}

impl Quote {
    /// Check for "empty" quotes - i.e. all 0's with no swaps. Balancer SOR API
    /// returns this in case it fails to find a route for whatever reason (not
    /// enough liquidity, no trading path, etc.). We don't consider this an
    /// error case.
    pub fn is_empty(&self) -> bool {
        *self == Quote::default()
    }
}

/// A swap included in a larger batched swap.
#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    /// The ID of the pool swapping in this step.
    pub pool_id: H256,
    /// The index in `token_addresses` for the input token.
    pub asset_in_index: usize,
    /// The index in `token_addresses` for the ouput token.
    pub asset_out_index: usize,
    /// The amount to swap.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,
    /// Additional user data to pass to the pool.
    #[serde_as(as = "serialize::Hex")]
    pub user_data: Vec<u8>,
}

/// Balancer SOR responds with `address: ""` on error cases.
mod address_default_when_empty {
    use {
        ethereum_types::H160,
        serde::{de, Deserialize as _, Deserializer},
        std::borrow::Cow,
    };

    pub fn deserialize<'de, D>(deserializer: D) -> Result<H160, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Cow::<str>::deserialize(deserializer)?;
        if value == "" {
            return Ok(H160::default());
        }
        value.parse().map_err(de::Error::custom)
    }
}

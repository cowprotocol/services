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
    /// The returned token amount.
    ///
    /// In buy token for sell orders or sell token for buy orders.
    #[serde_as(as = "serialize::U256")]
    pub return_amount: U256,
    /// The input (sell) token.
    #[serde(with = "address_default_when_empty")]
    pub token_in: H160,
    /// The output (buy) token.
    #[serde(with = "address_default_when_empty")]
    pub token_out: H160,
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
    #[serde(with = "value_or_string")]
    pub asset_in_index: usize,
    /// The index in `token_addresses` for the ouput token.
    #[serde(with = "value_or_string")]
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

/// Tries to either parse the `T` directly or tries to convert the value in case
/// it's a string. This is intended for deserializing number/string but is
/// generic enough to be used for any value that can be converted from a string.
mod value_or_string {
    use {
        serde::{de, Deserialize, Deserializer},
        std::borrow::Cow,
    };

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de> + std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum Content<'a, T> {
            Value(T),
            String(Cow<'a, str>),
        }

        match <Content<T>>::deserialize(deserializer) {
            Ok(Content::Value(value)) => Ok(value),
            Ok(Content::String(s)) => s.parse().map_err(de::Error::custom),
            Err(err) => Err(err),
        }
    }
}

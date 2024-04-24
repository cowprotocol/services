use {
    crate::domain::{
        competition::{
            self,
            auction::{self},
            order::{self, fees},
            Auction,
        },
        eth::{self},
        liquidity,
        time,
    },
    app_data::AppDataHash,
    model::order::OrderUid,
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
    std::collections::BTreeMap,
};

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionWithLiquidity {
    prices: BTreeMap<H160, Token>,
    gas_price: GasPrice,
    deadline: Deadline,
    orders: Vec<Order>,
    liquidity: Vec<Liquidity>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Deadline {
    driver: i64,
    solvers: i64,
}

impl From<time::Deadline> for Deadline {
    fn from(value: time::Deadline) -> Self {
        Self {
            driver: value.driver().timestamp(),
            solvers: value.solvers().timestamp(),
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Token {
    decimals: Option<u8>,
    symbol: Option<String>,
    address: H160,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    price: Option<U256>,
    #[serde_as(as = "HexOrDecimalU256")]
    available_balance: U256,
    trusted: bool,
}

impl From<auction::Token> for Token {
    fn from(value: auction::Token) -> Self {
        Self {
            decimals: value.decimals,
            symbol: value.symbol,
            address: value.address.into(),
            price: value.price.map(Into::into),
            available_balance: value.available_balance,
            trusted: value.trusted,
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GasPrice {
    #[serde_as(as = "HexOrDecimalU256")]
    max: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    tip: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    base: U256,
}

impl From<eth::GasPrice> for GasPrice {
    fn from(value: eth::GasPrice) -> Self {
        Self {
            max: value.max().into(),
            tip: value.tip().into(),
            base: value.base().into(),
        }
    }
}

impl AuctionWithLiquidity {
    pub fn build(auction: &Auction, liquidity: &[liquidity::Liquidity]) -> Self {
        Self {
            prices: auction
                .tokens()
                .iter()
                .cloned()
                .map(|token| (token.address.into(), token.into()))
                .collect::<BTreeMap<_, _>>(),
            gas_price: auction.gas_price().into(),
            deadline: auction.deadline().into(),
            orders: auction.orders().iter().cloned().map(Into::into).collect(),
            liquidity: liquidity.iter().cloned().map(Into::into).collect(),
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Liquidity {
    id: usize,
    gas: U256,
    kind: String,
}

impl From<liquidity::Liquidity> for Liquidity {
    fn from(value: crate::domain::Liquidity) -> Self {
        Self {
            id: value.id.into(),
            gas: value.gas.into(),
            kind: {
                let kind: &'static str = (&value.kind).into();
                kind.to_string()
            },
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Order {
    uid: OrderUid,
    sell_token: H160,
    buy_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    buy_amount: U256,
    protocol_fees: Vec<FeePolicy>,
    valid_to: u32,
    side: Side,
    kind: Kind,
    receiver: Option<H160>,
    partial: Partial,
    pre_interactions: Vec<Interaction>,
    post_interactions: Vec<Interaction>,
    sell_token_balance: SellTokenSource,
    buy_token_balance: BuyTokenDestination,
    app_data: AppDataHash,
    #[serde(flatten)]
    signature: Signature,
}

impl From<competition::Order> for Order {
    fn from(value: competition::Order) -> Self {
        Self {
            uid: OrderUid(value.uid.0 .0),
            sell_token: value.sell.token.into(),
            buy_token: value.buy.token.into(),
            sell_amount: value.sell.amount.into(),
            buy_amount: value.buy.amount.into(),
            protocol_fees: value.protocol_fees.into_iter().map(Into::into).collect(),
            valid_to: value.valid_to.into(),
            kind: value.kind.into(),
            side: value.side.into(),
            receiver: value.receiver.map(Into::into),
            partial: value.partial.into(),
            pre_interactions: value.pre_interactions.into_iter().map(Into::into).collect(),
            post_interactions: value
                .post_interactions
                .into_iter()
                .map(Into::into)
                .collect(),
            sell_token_balance: value.sell_token_balance.into(),
            buy_token_balance: value.buy_token_balance.into(),
            app_data: AppDataHash(value.app_data.0 .0),
            signature: Signature {
                signing_scheme: value.signature.scheme.into(),
                signature: value.signature.data.into(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Side {
    Buy,
    Sell,
}

impl From<order::Side> for Side {
    fn from(value: order::Side) -> Self {
        match value {
            order::Side::Buy => Self::Buy,
            order::Side::Sell => Self::Sell,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SellTokenSource {
    Erc20,
    External,
    Internal,
}

impl From<order::SellTokenBalance> for SellTokenSource {
    fn from(value: order::SellTokenBalance) -> Self {
        match value {
            order::SellTokenBalance::Erc20 => Self::Erc20,
            order::SellTokenBalance::Internal => Self::Internal,
            order::SellTokenBalance::External => Self::External,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum BuyTokenDestination {
    Erc20,
    Internal,
}

impl From<order::BuyTokenBalance> for BuyTokenDestination {
    fn from(value: order::BuyTokenBalance) -> Self {
        match value {
            order::BuyTokenBalance::Erc20 => Self::Erc20,
            order::BuyTokenBalance::Internal => Self::Internal,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum Partial {
    Yes { available: U256 },
    No,
}

impl From<order::Partial> for Partial {
    fn from(value: order::Partial) -> Self {
        match value {
            order::Partial::Yes { available } => Self::Yes {
                available: available.into(),
            },
            order::Partial::No => Self::No,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum FeePolicy {
    #[serde(rename_all = "camelCase")]
    Surplus { factor: f64, max_volume_factor: f64 },
    #[serde(rename_all = "camelCase")]
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote,
    },
    #[serde(rename_all = "camelCase")]
    Volume { factor: f64 },
}

impl From<fees::FeePolicy> for FeePolicy {
    fn from(value: order::FeePolicy) -> Self {
        match value {
            order::FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => Self::Surplus {
                factor,
                max_volume_factor,
            },
            order::FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => Self::PriceImprovement {
                factor,
                max_volume_factor,
                quote: quote.into(),
            },
            order::FeePolicy::Volume { factor } => Self::Volume { factor },
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Quote {
    #[serde_as(as = "HexOrDecimalU256")]
    sell_amount: U256,
    sell_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    buy_amount: U256,
    buy_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    fee_amount: U256,
    fee_token: H160,
}

impl From<fees::Quote> for Quote {
    fn from(value: fees::Quote) -> Self {
        Self {
            sell_amount: value.sell.amount.into(),
            sell_token: value.sell.token.into(),
            buy_amount: value.buy.amount.into(),
            buy_token: value.buy.token.into(),
            fee_amount: value.fee.amount.into(),
            fee_token: value.fee.token.into(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Market,
    Limit,
    Liquidity,
}

impl From<order::Kind> for Kind {
    fn from(value: order::Kind) -> Self {
        match value {
            order::Kind::Market => Self::Market,
            order::Kind::Limit => Self::Limit,
            order::Kind::Liquidity => Self::Liquidity,
        }
    }
}

/// Signature over the order data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Signature {
    signing_scheme: Scheme,
    #[serde(with = "bytes_hex")]
    signature: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum Scheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

impl From<order::signature::Scheme> for Scheme {
    fn from(value: order::signature::Scheme) -> Self {
        match value {
            order::signature::Scheme::Eip712 => Self::Eip1271,
            order::signature::Scheme::EthSign => Self::EthSign,
            order::signature::Scheme::Eip1271 => Self::Eip712,
            order::signature::Scheme::PreSign => Self::PreSign,
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Interaction {
    target: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    value: U256,
    #[serde(with = "bytes_hex")]
    call_data: Vec<u8>,
}

impl From<eth::Interaction> for Interaction {
    fn from(value: eth::Interaction) -> Self {
        Self {
            target: value.target.into(),
            value: value.value.into(),
            call_data: value.call_data.into(),
        }
    }
}

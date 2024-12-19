use {
    crate::{
        boundary::{self},
        domain::{self, eth, fee::FeeFactor, OrderUid},
    },
    app_data::AppDataHash,
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub uid: boundary::OrderUid,
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    pub protocol_fees: Vec<FeePolicy>,
    pub created: u32,
    pub valid_to: u32,
    pub kind: boundary::OrderKind,
    pub receiver: Option<H160>,
    pub owner: H160,
    pub partially_fillable: bool,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed: U256,
    pub pre_interactions: Vec<boundary::InteractionData>,
    pub post_interactions: Vec<boundary::InteractionData>,
    pub sell_token_balance: boundary::SellTokenSource,
    pub buy_token_balance: boundary::BuyTokenDestination,
    #[serde(flatten)]
    pub class: boundary::OrderClass,
    pub app_data: AppDataHash,
    #[serde(flatten)]
    pub signature: boundary::Signature,
    pub quote: Option<Quote>,
}

pub fn from_domain(order: domain::Order) -> Order {
    Order {
        uid: order.uid.into(),
        sell_token: order.sell.token.into(),
        buy_token: order.buy.token.into(),
        sell_amount: order.sell.amount.into(),
        buy_amount: order.buy.amount.into(),
        protocol_fees: order
            .protocol_fees
            .into_iter()
            .map(FeePolicy::from_domain)
            .collect(),
        created: order.created,
        valid_to: order.valid_to,
        kind: order.side.into(),
        receiver: order.receiver.map(Into::into),
        owner: order.owner.into(),
        partially_fillable: order.partially_fillable,
        executed: order.executed.into(),
        pre_interactions: order.pre_interactions.into_iter().map(Into::into).collect(),
        post_interactions: order
            .post_interactions
            .into_iter()
            .map(Into::into)
            .collect(),
        sell_token_balance: order.sell_token_balance.into(),
        buy_token_balance: order.buy_token_balance.into(),
        class: boundary::OrderClass::Limit,
        app_data: order.app_data.into(),
        signature: order.signature.into(),
        quote: order.quote.map(Quote::from_domain),
    }
}

pub fn to_domain(order: Order) -> domain::Order {
    domain::Order {
        uid: order.uid.into(),
        sell: eth::Asset {
            token: order.sell_token.into(),
            amount: order.sell_amount.into(),
        },
        buy: eth::Asset {
            token: order.buy_token.into(),
            amount: order.buy_amount.into(),
        },
        protocol_fees: order
            .protocol_fees
            .into_iter()
            .map(FeePolicy::into_domain)
            .collect(),
        created: order.created,
        valid_to: order.valid_to,
        side: order.kind.into(),
        receiver: order.receiver.map(Into::into),
        owner: order.owner.into(),
        partially_fillable: order.partially_fillable,
        executed: order.executed.into(),
        pre_interactions: order.pre_interactions.into_iter().map(Into::into).collect(),
        post_interactions: order
            .post_interactions
            .into_iter()
            .map(Into::into)
            .collect(),
        sell_token_balance: order.sell_token_balance.into(),
        buy_token_balance: order.buy_token_balance.into(),
        app_data: order.app_data.into(),
        signature: order.signature.into(),
        quote: order.quote.map(|q| q.to_domain(order.uid.into())),
    }
}

impl From<boundary::OrderUid> for domain::OrderUid {
    fn from(uid: boundary::OrderUid) -> Self {
        Self(uid.0)
    }
}

impl From<domain::OrderUid> for boundary::OrderUid {
    fn from(uid: domain::OrderUid) -> Self {
        Self(uid.0)
    }
}

impl From<domain::auction::order::Side> for boundary::OrderKind {
    fn from(kind: domain::auction::order::Side) -> Self {
        match kind {
            domain::auction::order::Side::Buy => Self::Buy,
            domain::auction::order::Side::Sell => Self::Sell,
        }
    }
}

impl From<boundary::OrderKind> for domain::auction::order::Side {
    fn from(kind: boundary::OrderKind) -> Self {
        match kind {
            boundary::OrderKind::Buy => Self::Buy,
            boundary::OrderKind::Sell => Self::Sell,
        }
    }
}

impl From<domain::auction::order::Interaction> for boundary::InteractionData {
    fn from(interaction: domain::auction::order::Interaction) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            call_data: interaction.call_data,
        }
    }
}

impl From<boundary::InteractionData> for domain::auction::order::Interaction {
    fn from(interaction: boundary::InteractionData) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            call_data: interaction.call_data,
        }
    }
}

impl From<domain::auction::order::SellTokenSource> for boundary::SellTokenSource {
    fn from(source: domain::auction::order::SellTokenSource) -> Self {
        match source {
            domain::auction::order::SellTokenSource::Erc20 => Self::Erc20,
            domain::auction::order::SellTokenSource::External => Self::External,
            domain::auction::order::SellTokenSource::Internal => Self::Internal,
        }
    }
}

impl From<boundary::SellTokenSource> for domain::auction::order::SellTokenSource {
    fn from(source: boundary::SellTokenSource) -> Self {
        match source {
            boundary::SellTokenSource::Erc20 => Self::Erc20,
            boundary::SellTokenSource::External => Self::External,
            boundary::SellTokenSource::Internal => Self::Internal,
        }
    }
}

impl From<domain::auction::order::BuyTokenDestination> for boundary::BuyTokenDestination {
    fn from(destination: domain::auction::order::BuyTokenDestination) -> Self {
        match destination {
            domain::auction::order::BuyTokenDestination::Erc20 => Self::Erc20,
            domain::auction::order::BuyTokenDestination::Internal => Self::Internal,
        }
    }
}

impl From<boundary::BuyTokenDestination> for domain::auction::order::BuyTokenDestination {
    fn from(destination: boundary::BuyTokenDestination) -> Self {
        match destination {
            boundary::BuyTokenDestination::Erc20 => Self::Erc20,
            boundary::BuyTokenDestination::Internal => Self::Internal,
        }
    }
}

impl From<domain::auction::order::AppDataHash> for AppDataHash {
    fn from(hash: domain::auction::order::AppDataHash) -> Self {
        Self(hash.0)
    }
}

impl From<AppDataHash> for domain::auction::order::AppDataHash {
    fn from(hash: AppDataHash) -> Self {
        Self(hash.0)
    }
}

impl From<domain::auction::order::Signature> for boundary::Signature {
    fn from(signature: domain::auction::order::Signature) -> Self {
        match signature {
            domain::auction::order::Signature::Eip712(s) => Self::Eip712(s.into()),
            domain::auction::order::Signature::EthSign(s) => Self::EthSign(s.into()),
            domain::auction::order::Signature::Eip1271(b) => Self::Eip1271(b),
            domain::auction::order::Signature::PreSign => Self::PreSign,
        }
    }
}

impl From<boundary::Signature> for domain::auction::order::Signature {
    fn from(signature: boundary::Signature) -> Self {
        match signature {
            boundary::Signature::Eip712(s) => Self::Eip712(s.into()),
            boundary::Signature::EthSign(s) => Self::EthSign(s.into()),
            boundary::Signature::Eip1271(b) => Self::Eip1271(b),
            boundary::Signature::PreSign => Self::PreSign,
        }
    }
}

impl From<domain::auction::order::EcdsaSignature> for boundary::EcdsaSignature {
    fn from(signature: domain::auction::order::EcdsaSignature) -> Self {
        Self {
            r: signature.r,
            s: signature.s,
            v: signature.v,
        }
    }
}

impl From<boundary::EcdsaSignature> for domain::auction::order::EcdsaSignature {
    fn from(signature: boundary::EcdsaSignature) -> Self {
        Self {
            r: signature.r,
            s: signature.s,
            v: signature.v,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeePolicy {
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

impl FeePolicy {
    pub fn from_domain(policy: domain::fee::Policy) -> Self {
        match policy {
            domain::fee::Policy::Surplus {
                factor,
                max_volume_factor,
            } => Self::Surplus {
                factor: factor.into(),
                max_volume_factor: max_volume_factor.into(),
            },
            domain::fee::Policy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => Self::PriceImprovement {
                factor: factor.into(),
                max_volume_factor: max_volume_factor.into(),
                quote: Quote {
                    sell_amount: quote.sell_amount,
                    buy_amount: quote.buy_amount,
                    fee: quote.fee,
                    solver: quote.solver,
                },
            },
            domain::fee::Policy::Volume { factor } => Self::Volume {
                factor: factor.into(),
            },
        }
    }

    pub fn into_domain(self) -> domain::fee::Policy {
        match self {
            Self::Surplus {
                factor,
                max_volume_factor,
            } => domain::fee::Policy::Surplus {
                factor: FeeFactor::try_from(factor).unwrap(),
                max_volume_factor: FeeFactor::try_from(max_volume_factor).unwrap(),
            },
            Self::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => domain::fee::Policy::PriceImprovement {
                factor: FeeFactor::try_from(factor).unwrap(),
                max_volume_factor: FeeFactor::try_from(max_volume_factor).unwrap(),
                quote: domain::fee::Quote {
                    sell_amount: quote.sell_amount,
                    buy_amount: quote.buy_amount,
                    fee: quote.fee,
                    solver: quote.solver,
                },
            },
            Self::Volume { factor } => domain::fee::Policy::Volume {
                factor: FeeFactor::try_from(factor).unwrap(),
            },
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub fee: U256,
    pub solver: H160,
}

impl Quote {
    fn from_domain(quote: domain::Quote) -> Self {
        Quote {
            sell_amount: quote.sell_amount.0,
            buy_amount: quote.buy_amount.0,
            fee: quote.fee.0,
            solver: quote.solver.0,
        }
    }

    pub fn to_domain(&self, order_uid: OrderUid) -> domain::Quote {
        domain::Quote {
            order_uid,
            sell_amount: self.sell_amount.into(),
            buy_amount: self.buy_amount.into(),
            fee: self.fee.into(),
            solver: self.solver.into(),
        }
    }
}

impl From<domain::auction::order::Side> for database::orders::OrderKind {
    fn from(side: domain::auction::order::Side) -> Self {
        match side {
            domain::auction::order::Side::Buy => Self::Buy,
            domain::auction::order::Side::Sell => Self::Sell,
        }
    }
}

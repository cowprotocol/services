use {
    crate::{
        boundary::{self},
        domain,
    },
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    shared::app_data::Validator,
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
    #[serde_as(as = "HexOrDecimalU256")]
    pub user_fee: U256,
    pub protocol_fees: Vec<FeePolicy>,
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
    pub app_data: boundary::AppDataHash,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_app_data: Option<String>,
    #[serde(flatten)]
    pub signature: boundary::Signature,
}

pub fn from_domain(order: domain::Order) -> Order {
    Order {
        uid: order.uid.into(),
        sell_token: order.sell_token,
        buy_token: order.buy_token,
        sell_amount: order.sell_amount,
        buy_amount: order.buy_amount,
        user_fee: order.user_fee,
        protocol_fees: order.protocol_fees.into_iter().map(Into::into).collect(),
        valid_to: order.valid_to,
        kind: order.side.into(),
        receiver: order.receiver,
        owner: order.owner,
        partially_fillable: order.partially_fillable,
        executed: order.executed,
        pre_interactions: order.pre_interactions.into_iter().map(Into::into).collect(),
        post_interactions: order
            .post_interactions
            .into_iter()
            .map(Into::into)
            .collect(),
        sell_token_balance: order.sell_token_balance.into(),
        buy_token_balance: order.buy_token_balance.into(),
        class: order.class.into(),
        app_data: order.app_data.into(),
        full_app_data: order
            .full_app_data
            .map(|full_app_data| full_app_data.document),
        signature: order.signature.into(),
    }
}

pub fn to_domain(order: Order) -> domain::Order {
    domain::Order {
        uid: order.uid.into(),
        sell_token: order.sell_token,
        buy_token: order.buy_token,
        sell_amount: order.sell_amount,
        buy_amount: order.buy_amount,
        user_fee: order.user_fee,
        protocol_fees: order.protocol_fees.into_iter().map(Into::into).collect(),
        valid_to: order.valid_to,
        side: order.kind.into(),
        receiver: order.receiver,
        owner: order.owner,
        partially_fillable: order.partially_fillable,
        executed: order.executed,
        pre_interactions: order.pre_interactions.into_iter().map(Into::into).collect(),
        post_interactions: order
            .post_interactions
            .into_iter()
            .map(Into::into)
            .collect(),
        sell_token_balance: order.sell_token_balance.into(),
        buy_token_balance: order.buy_token_balance.into(),
        class: order.class.into(),
        app_data: order.app_data.into(),
        full_app_data: order.full_app_data.and_then(|full_app_data| {
            Validator::new(usize::MAX)
                .validate(full_app_data.as_bytes())
                .ok()
        }),
        signature: order.signature.into(),
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

impl From<domain::auction::order::Class> for boundary::OrderClass {
    fn from(class: domain::auction::order::Class) -> Self {
        match class {
            domain::auction::order::Class::Limit => Self::Limit,
            domain::auction::order::Class::Market => Self::Market,
            domain::auction::order::Class::Liquidity => Self::Liquidity,
        }
    }
}

impl From<boundary::OrderClass> for domain::auction::order::Class {
    fn from(class: boundary::OrderClass) -> Self {
        match class {
            boundary::OrderClass::Limit => Self::Limit,
            boundary::OrderClass::Market => Self::Market,
            boundary::OrderClass::Liquidity => Self::Liquidity,
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

impl From<domain::auction::order::AppDataHash> for boundary::AppDataHash {
    fn from(hash: domain::auction::order::AppDataHash) -> Self {
        Self(hash.0)
    }
}

impl From<boundary::AppDataHash> for domain::auction::order::AppDataHash {
    fn from(hash: boundary::AppDataHash) -> Self {
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

#[serde_as]
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

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee: U256,
}

impl From<domain::fee::Policy> for FeePolicy {
    fn from(policy: domain::fee::Policy) -> Self {
        match policy {
            domain::fee::Policy::Surplus {
                factor,
                max_volume_factor,
            } => Self::Surplus {
                factor,
                max_volume_factor,
            },
            domain::fee::Policy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => Self::PriceImprovement {
                factor,
                max_volume_factor,
                quote: Quote {
                    sell_amount: quote.sell_amount,
                    buy_amount: quote.buy_amount,
                    fee: quote.fee,
                },
            },
            domain::fee::Policy::Volume { factor } => Self::Volume { factor },
        }
    }
}

impl From<FeePolicy> for domain::fee::Policy {
    fn from(policy: FeePolicy) -> Self {
        match policy {
            FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => Self::Surplus {
                factor,
                max_volume_factor,
            },
            FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => Self::PriceImprovement {
                factor,
                max_volume_factor,
                quote: domain::fee::Quote {
                    sell_amount: quote.sell_amount,
                    buy_amount: quote.buy_amount,
                    fee: quote.fee,
                },
            },
            FeePolicy::Volume { factor } => Self::Volume { factor },
        }
    }
}

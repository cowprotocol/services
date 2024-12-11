use {
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
};

#[serde_as]
#[derive(PartialEq, Clone, Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub enum FeePolicy {
    #[serde(rename_all = "camelCase")]
    Surplus { factor: f64, max_volume_factor: f64 },
    #[serde(rename_all = "camelCase")]
    Volume { factor: f64 },
    #[serde(rename_all = "camelCase")]
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote,
    },
}

#[serde_as]
#[derive(PartialEq, Clone, Debug, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
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

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct ExecutedProtocolFee {
    pub policy: FeePolicy,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
    pub token: H160,
}

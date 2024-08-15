use {
    crate::domain::eth,
    number::serialization::HexOrDecimalU256,
    serde_json::json,
    serde_with::serde_as,
};

#[serde_as]
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub struct Quote {
    /// How much tokens the trader is expected to buy
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy: eth::U256,
    /// How much tokens the user is expected to sell (excluding network fee)
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell: eth::U256,
    /// The expected network fee, which is expected to be taken as
    /// additional sell amount.
    #[serde_as(as = "HexOrDecimalU256")]
    pub network_fee: eth::U256,
    pub solver: eth::H160,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Policy {
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

impl Policy {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Policy::Surplus {
                factor,
                max_volume_factor,
            } => json!({
                "surplus": {
                    "factor": factor,
                    "maxVolumeFactor": max_volume_factor
                }
            }),
            Policy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => json!({
                "priceImprovement": {
                    "factor": factor,
                    "maxVolumeFactor": max_volume_factor,
                    "quote": {
                        "sellAmount": quote.sell.to_string(),
                        "buyAmount": quote.buy.to_string(),
                        "fee": quote.network_fee.to_string(),
                        "solver": quote.solver,
                    }
                }
            }),
            Policy::Volume { factor } => json!({
                "volume": {
                    "factor": factor
                }
            }),
        }
    }
}

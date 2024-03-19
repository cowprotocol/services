use {crate::domain::eth, serde_json::json, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub struct Quote {
    /// How much tokens the trader is expected to buy
    pub buy: eth::U256,
    /// How much tokens the user is expected to sell (excluding network fee)
    pub sell: eth::U256,
    /// The expected network fee, which is expected to be taken as
    /// additional sell amount.
    pub network_fee: eth::U256,
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
                        "sellAmount": quote.sell,
                        "buyAmount": quote.buy,
                        "fee": quote.network_fee,
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

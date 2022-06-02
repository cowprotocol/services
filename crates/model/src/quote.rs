use crate::{
    app_id::AppId,
    order::{BuyTokenDestination, OrderKind, SellTokenSource},
    signature::SigningScheme,
    u256_decimal,
};
use chrono::{DateTime, Utc};
use primitive_types::{H160, U256};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PriceQuality {
    Fast,
    Optimal,
}

impl Default for PriceQuality {
    fn default() -> Self {
        Self::Optimal
    }
}

/// The order parameters to quote a price and fee for.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuoteRequest {
    pub from: H160,
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<H160>,
    #[serde(flatten)]
    pub side: OrderQuoteSide,
    #[serde(
        default,
        deserialize_with = "deserialize_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub valid_to: Option<u32>,
    #[serde(default)]
    pub app_data: AppId,
    #[serde(default)]
    pub partially_fillable: bool,
    #[serde(default)]
    pub sell_token_balance: SellTokenSource,
    #[serde(default)]
    pub buy_token_balance: BuyTokenDestination,
    #[serde(default)]
    pub signing_scheme: SigningScheme,
    #[serde(default)]
    pub price_quality: PriceQuality,
}

fn deserialize_non_null<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OrderQuoteSide {
    #[serde(rename_all = "camelCase")]
    Sell {
        #[serde(flatten)]
        sell_amount: SellAmount,
    },
    #[serde(rename_all = "camelCase")]
    Buy {
        #[serde(with = "u256_decimal")]
        buy_amount_after_fee: U256,
    },
}

impl Default for OrderQuoteSide {
    fn default() -> Self {
        Self::Buy {
            buy_amount_after_fee: U256::one(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum SellAmount {
    BeforeFee {
        #[serde(rename = "sellAmountBeforeFee", with = "u256_decimal")]
        value: U256,
    },
    AfterFee {
        #[serde(rename = "sellAmountAfterFee", with = "u256_decimal")]
        value: U256,
    },
}

/// The quoted order by the service.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuote {
    pub sell_token: H160,
    pub buy_token: H160,
    pub receiver: Option<H160>,
    #[serde(with = "u256_decimal")]
    pub sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub buy_amount: U256,
    pub valid_to: u32,
    pub app_data: AppId,
    #[serde(with = "u256_decimal")]
    pub fee_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
}

pub type QuoteId = u64;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuoteResponse {
    pub quote: OrderQuote,
    pub from: H160,
    pub expiration: DateTime<Utc>,
    pub id: QuoteId,
}

impl OrderQuoteRequest {
    /// This method is used by the old, deprecated, fee endpoint to convert {Buy, Sell}Requests
    pub fn new(sell_token: H160, buy_token: H160, side: OrderQuoteSide) -> Self {
        Self {
            sell_token,
            buy_token,
            side,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_defaults() {
        assert_eq!(
            json!(OrderQuoteRequest::default()),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000000",
                "buyToken": "0x0000000000000000000000000000000000000000",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "partiallyFillable": false,
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20",
                "signingScheme": "eip712",
                "priceQuality": "optimal",
            })
        );
    }

    #[test]
    fn deserialize_denies_null_valid_to() {
        assert!(serde_json::from_value::<OrderQuoteRequest>(json!({
            "from": "0x0000000000000000000000000000000000000000",
            "sellToken": "0x0000000000000000000000000000000000000000",
            "buyToken": "0x0000000000000000000000000000000000000000",
            "kind": "buy",
            "buyAmountAfterFee": "1",
            "validTo": null,
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "signingScheme": "eip712",
            "priceQuality": "optimal",
        }))
        .is_err());
    }
}

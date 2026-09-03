//! Wire shape of the quote endpoint, mirroring the EVM orderbook.

use {
    chrono::{DateTime, Utc},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
};

/// A quote request. Unknown fields are ignored rather than rejected: the EVM
/// request body carries fields with no Solana meaning.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Wallet the quoted order would belong to.
    #[serde_as(as = "DisplayFromStr")]
    pub from: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token: Pubkey,
    /// The buy token account proceeds would land in. Echoed back untouched.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub receiver: Option<Pubkey>,
    #[serde(flatten)]
    pub side: Side,
    #[serde(flatten, default)]
    pub validity: Option<Validity>,
    /// `0x`-prefixed 32 bytes, echoed back untouched.
    #[serde(default)]
    pub app_data: Option<String>,
}

/// Which side the quote fixes, and the amount that side names.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Side {
    Sell {
        #[serde(flatten)]
        sell_amount: SellAmount,
    },
    #[serde(rename_all = "camelCase")]
    Buy {
        #[serde_as(as = "DisplayFromStr")]
        buy_amount_after_fee: u64,
    },
}

/// The sell amount, with or without the fee. No component charges a fee, so
/// both spellings name the same number.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SellAmount {
    BeforeFee {
        #[serde(rename = "sellAmountBeforeFee")]
        #[serde_as(as = "DisplayFromStr")]
        value: u64,
    },
    AfterFee {
        #[serde(rename = "sellAmountAfterFee")]
        #[serde_as(as = "DisplayFromStr")]
        value: u64,
    },
}

/// How long the quoted order would stay valid.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Validity {
    ValidTo(u32),
    ValidFor(u32),
}

impl Side {
    /// The side and the amount it fixes.
    pub fn kind_and_amount(&self) -> (Kind, u64) {
        match self {
            Self::Sell {
                sell_amount: SellAmount::BeforeFee { value } | SellAmount::AfterFee { value },
            } => (Kind::Sell, *value),
            Self::Buy {
                buy_amount_after_fee,
            } => (Kind::Buy, *buy_amount_after_fee),
        }
    }
}

/// Which amount the order fixes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Sell,
    Buy,
}

/// A quote response.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub quote: Quote,
    #[serde_as(as = "DisplayFromStr")]
    pub from: Pubkey,
    /// When the quoted amounts stop being honored.
    pub expiration: DateTime<Utc>,
    /// The quote's database id. Always absent: quotes are not persisted.
    pub id: Option<i64>,
    /// Whether the amounts were confirmed by simulating the settlement. No
    /// component simulates, so a quote is indicative.
    pub verified: bool,
}

/// The quoted order.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token: Pubkey,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub receiver: Option<Pubkey>,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: u64,
    /// Unix seconds.
    pub valid_to: u32,
    pub app_data: Option<String>,
    /// Always zero: no component charges a fee.
    #[serde_as(as = "DisplayFromStr")]
    pub fee_amount: u64,
    pub kind: Kind,
    pub partially_fillable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_evm_request_body() {
        // Fields with no Solana meaning.
        let raw = serde_json::json!({
            "from": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
            "sellToken": "So11111111111111111111111111111111111111112",
            "buyToken": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "kind": "sell",
            "sellAmountBeforeFee": "10000000",
            "validFor": 1800,
            "signingScheme": "eip712",
            "priceQuality": "optimal",
            "sellTokenBalance": "erc20"
        });
        let request: Request = serde_json::from_value(raw).unwrap();
        assert_eq!(request.side.kind_and_amount(), (Kind::Sell, 10_000_000));
        assert!(matches!(request.validity, Some(Validity::ValidFor(1800))));
        assert!(request.receiver.is_none());
    }

    #[test]
    fn accepts_a_buy_request() {
        let raw = serde_json::json!({
            "from": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
            "sellToken": "So11111111111111111111111111111111111111112",
            "buyToken": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "kind": "buy",
            "buyAmountAfterFee": "1000000",
            "validTo": 1787740563
        });
        let request: Request = serde_json::from_value(raw).unwrap();
        assert_eq!(request.side.kind_and_amount(), (Kind::Buy, 1_000_000));
        assert!(matches!(
            request.validity,
            Some(Validity::ValidTo(1787740563))
        ));
    }

    /// Both spellings of the sell amount name the same number.
    #[test]
    fn sell_amount_after_fee_is_read_too() {
        let raw = serde_json::json!({
            "from": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
            "sellToken": "So11111111111111111111111111111111111111112",
            "buyToken": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "kind": "sell",
            "sellAmountAfterFee": "42"
        });
        let request: Request = serde_json::from_value(raw).unwrap();
        assert_eq!(request.side.kind_and_amount(), (Kind::Sell, 42));
        assert!(request.validity.is_none());
    }
}

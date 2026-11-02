//! Wire shape of the quote endpoint, mirroring the EVM orderbook.

use {
    app_data::OrderCreationAppData,
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
    /// Account or wallet the proceeds would land in. Echoed back untouched.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub receiver: Option<Pubkey>,
    #[serde(flatten)]
    pub side: Side,
    #[serde(flatten, default)]
    pub validity: Option<Validity>,
    /// The order's app data, the document or its hash, checked like at
    /// placement. The zero hash when absent.
    #[serde(flatten, deserialize_with = "app_data::deserialize_optional_app_data")]
    pub app_data: OrderCreationAppData,
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
    /// The stored quote's id, absent when the store was unavailable.
    pub id: Option<i64>,
    /// Whether the amounts were confirmed by simulating the settlement.
    pub verified: bool,
    /// The account a sponsored creation transaction must name as its fee payer.
    /// Absent when the deployment does not sponsor placements.
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub funder: Option<Pubkey>,
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
    /// The request's app data, a document with its hash alongside.
    #[serde(flatten)]
    pub app_data: OrderCreationAppData,
    /// Always zero: no component charges a fee.
    #[serde_as(as = "DisplayFromStr")]
    pub fee_amount: u64,
    pub costs: Costs,
    pub kind: Kind,
    pub partially_fillable: bool,
}

/// The execution cost taken out of the quoted amounts, in the token it comes
/// out of.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Costs {
    /// The buy token of a sell order, the sell token of a buy order. The
    /// amounts below are in its atoms.
    #[serde_as(as = "DisplayFromStr")]
    pub token: Pubkey,
    /// The network fee of settling the quoted route, as the winning driver
    /// reported it, zero when it reported none.
    #[serde_as(as = "DisplayFromStr")]
    pub network_fees: u64,
    /// Rent of the buy token account when the settlement must create it.
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token_ata_costs: u64,
    /// Always zero: no component charges a fee.
    #[serde_as(as = "DisplayFromStr")]
    pub protocol_fee: u64,
    /// The sum: a sell order's `buy_amount` is lower by it, a buy order's
    /// `sell_amount` higher.
    #[serde_as(as = "DisplayFromStr")]
    pub total: u64,
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

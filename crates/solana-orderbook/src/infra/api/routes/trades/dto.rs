//! Wire shape of the trades endpoint.

use {
    crate::infra::db::{self, ExecutedFeeRow, TradeRow},
    anyhow::{Context, Result},
    bigdecimal::{BigDecimal, ToPrimitive},
    database::solana::PolicyKind,
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::{pubkey::Pubkey, signature::Signature},
};

/// One trade on the wire.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub order_uid: String,
    #[serde_as(as = "DisplayFromStr")]
    pub owner: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub tx_signature: Signature,
    /// Position of the settlement instruction within its transaction,
    /// disambiguating multiple fills of one order in the same transaction.
    pub instruction_index: i32,
    /// Slot the settlement landed in. Absent while the settlement row has
    /// not been indexed.
    pub slot: Option<i64>,
    /// The protocol fees this fill paid, in the order the policies applied.
    pub executed_protocol_fees: Vec<ExecutedProtocolFee>,
}

impl Trade {
    pub fn new(row: TradeRow, executed_protocol_fees: Vec<ExecutedProtocolFee>) -> Self {
        Self {
            order_uid: format!("0x{}", const_hex::encode(row.order_uid.0)),
            owner: Pubkey::new_from_array(row.owner.0),
            sell_token: Pubkey::new_from_array(row.sell_token.0),
            buy_token: Pubkey::new_from_array(row.buy_token.0),
            sell_amount: row.sell_amount,
            buy_amount: row.buy_amount,
            tx_signature: Signature::from(row.tx_signature.0),
            instruction_index: row.instruction_index,
            slot: row.slot,
            executed_protocol_fees,
        }
    }
}

/// A protocol fee a fill paid and the policy that took it.
#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutedProtocolFee {
    pub policy: FeePolicy,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub token: Pubkey,
}

/// A protocol fee policy, keyed by its kind.
#[derive(Clone, Debug, Serialize)]
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

/// The quote a price improvement fee measured against. Its amounts are net
/// of the quote's execution cost, so `fee` is zero.
#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub fee: u64,
}

impl TryFrom<ExecutedFeeRow> for ExecutedProtocolFee {
    type Error = anyhow::Error;

    fn try_from(row: ExecutedFeeRow) -> Result<Self> {
        let policy = match row.kind {
            PolicyKind::Surplus => FeePolicy::Surplus {
                factor: row.surplus_factor.context("missing surplus_factor")?,
                max_volume_factor: row
                    .surplus_max_volume_factor
                    .context("missing surplus_max_volume_factor")?,
            },
            PolicyKind::Volume => FeePolicy::Volume {
                factor: row.volume_factor.context("missing volume_factor")?,
            },
            PolicyKind::PriceImprovement => {
                // The linked quote net of its cost, or without one the
                // zero-buy reference quote, as the autopilot measured.
                let linked = match (
                    &row.quote_sell_amount,
                    &row.quote_buy_amount,
                    row.quote_execution_cost
                        .as_ref()
                        .and_then(ToPrimitive::to_u64),
                ) {
                    (Some(sell_amount), Some(buy_amount), Some(execution_cost)) => {
                        db::net_quote(row.order_kind, sell_amount, buy_amount, execution_cost)
                    }
                    _ => None,
                };
                let (sell_amount, buy_amount) = match linked {
                    Some(amounts) => amounts,
                    None => (
                        row.order_sell_amount
                            .to_u64()
                            .context("order sell_amount does not fit u64")?,
                        0,
                    ),
                };
                FeePolicy::PriceImprovement {
                    factor: row
                        .price_improvement_factor
                        .context("missing price_improvement_factor")?,
                    max_volume_factor: row
                        .price_improvement_max_volume_factor
                        .context("missing price_improvement_max_volume_factor")?,
                    quote: Quote {
                        sell_amount,
                        buy_amount,
                        fee: 0,
                    },
                }
            }
        };
        Ok(Self {
            policy,
            amount: row.amount,
            token: Pubkey::new_from_array(row.token.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        database::{
            byte_array::ByteArray,
            solana::{OrderKind, PolicyKind},
        },
        serde_json::json,
    };

    /// A fee of 5 in `[0x22; 32]` that a sell order of 1000 paid in auction
    /// 7, the order linked to a route quote `(sell, buy, execution cost)`.
    fn fee_row(kind: PolicyKind, quote: Option<(u64, u64, u64)>) -> ExecutedFeeRow {
        ExecutedFeeRow {
            auction_id: 7,
            order_uid: ByteArray([0x11; 32]),
            kind,
            surplus_factor: Some(0.5),
            surplus_max_volume_factor: Some(0.9),
            price_improvement_factor: Some(0.3),
            price_improvement_max_volume_factor: Some(0.01),
            volume_factor: Some(0.002),
            token: ByteArray([0x22; 32]),
            amount: 5.into(),
            order_kind: OrderKind::Sell,
            order_sell_amount: 1_000.into(),
            quote_sell_amount: quote.map(|(sell, _, _)| sell.into()),
            quote_buy_amount: quote.map(|(_, buy, _)| buy.into()),
            quote_execution_cost: quote.map(|(_, _, cost)| cost.into()),
        }
    }

    /// The fees keep the EVM `executedProtocolFees` shape: the policy keyed
    /// by its kind, decimal amounts and the token. A price improvement
    /// policy shows the quote it measured against, net of the quote's
    /// execution cost, or the zero-buy reference quote without a link.
    #[test]
    fn executed_fees_keep_the_evm_shape() {
        let fees = [
            fee_row(PolicyKind::Surplus, None),
            fee_row(PolicyKind::Volume, None),
            fee_row(PolicyKind::PriceImprovement, Some((1_000, 600, 50))),
            fee_row(PolicyKind::PriceImprovement, None),
        ]
        .map(|row| serde_json::to_value(ExecutedProtocolFee::try_from(row).unwrap()).unwrap());
        let token = "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3";
        assert_eq!(
            fees,
            [
                json!({
                    "policy": { "surplus": { "factor": 0.5, "maxVolumeFactor": 0.9 } },
                    "amount": "5",
                    "token": token,
                }),
                json!({
                    "policy": { "volume": { "factor": 0.002 } },
                    "amount": "5",
                    "token": token,
                }),
                json!({
                    "policy": { "priceImprovement": {
                        "factor": 0.3,
                        "maxVolumeFactor": 0.01,
                        "quote": { "sellAmount": "1000", "buyAmount": "550", "fee": "0" },
                    } },
                    "amount": "5",
                    "token": token,
                }),
                json!({
                    "policy": { "priceImprovement": {
                        "factor": 0.3,
                        "maxVolumeFactor": 0.01,
                        "quote": { "sellAmount": "1000", "buyAmount": "0", "fee": "0" },
                    } },
                    "amount": "5",
                    "token": token,
                }),
            ]
        );
    }

    #[test]
    fn wire_format_is_stable() {
        let trade = Trade::new(
            TradeRow {
                order_uid: ByteArray([0x11; 32]),
                owner: ByteArray([0x22; 32]),
                sell_token: ByteArray([0x33; 32]),
                buy_token: ByteArray([0x44; 32]),
                sell_amount: 1_000.into(),
                buy_amount: 500.into(),
                tx_signature: ByteArray([9; 64]),
                instruction_index: 3,
                slot: Some(42),
                auction_id: Some(7),
            },
            Vec::new(),
        );
        let json = serde_json::to_value(&trade).unwrap();
        assert_eq!(json["executedProtocolFees"], json!([]));
        assert_eq!(json["orderUid"], format!("0x{}", "11".repeat(32)));
        assert_eq!(
            json["owner"],
            "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3"
        );
        assert_eq!(json["sellAmount"], "1000");
        assert_eq!(json["buyAmount"], "500");
        assert_eq!(
            json["txSignature"],
            "BUguQsv2ZuHus54HAFzjdJHzZBkygAjKhEeYwSG19tUfUyvvz3worsdQCdAXDNjakJHioSiyxhFiDJrm8XpSXRA"
        );
        assert_eq!(json["instructionIndex"], 3);
        assert_eq!(json["slot"], 42);
    }
}

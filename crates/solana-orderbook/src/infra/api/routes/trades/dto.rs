//! Wire shape of the trades endpoint.

use {
    crate::infra::db::TradeRow,
    bigdecimal::BigDecimal,
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
}

impl From<TradeRow> for Trade {
    fn from(row: TradeRow) -> Self {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, database::byte_array::ByteArray};

    #[test]
    fn wire_format_is_stable() {
        let trade = Trade::from(TradeRow {
            order_uid: ByteArray([0x11; 32]),
            owner: ByteArray([0x22; 32]),
            sell_token: ByteArray([0x33; 32]),
            buy_token: ByteArray([0x44; 32]),
            sell_amount: 1_000.into(),
            buy_amount: 500.into(),
            tx_signature: ByteArray([9; 64]),
            instruction_index: 3,
            slot: Some(42),
        });
        let json = serde_json::to_value(&trade).unwrap();
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

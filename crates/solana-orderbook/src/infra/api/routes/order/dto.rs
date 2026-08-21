//! Wire shape of the order endpoint.

use {
    crate::infra::db::OrderRow,
    bigdecimal::BigDecimal,
    chrono::{DateTime, Utc},
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
};

/// One order with its fill state, camelCase on the wire, pubkeys as base58,
/// amounts as decimal strings, the uid as `0x`-hex.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub uid: String,
    #[serde_as(as = "DisplayFromStr")]
    pub owner: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token_account: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token_account: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: BigDecimal,
    /// Unix seconds.
    pub valid_to: i64,
    pub kind: String,
    pub partially_fillable: bool,
    pub app_data: String,
    #[serde_as(as = "DisplayFromStr")]
    pub order_pda: Pubkey,
    pub creation_date: DateTime<Utc>,
    /// Sell tokens pulled from the order so far.
    #[serde_as(as = "DisplayFromStr")]
    pub executed_sell_amount: BigDecimal,
    /// Buy tokens pushed to the order so far.
    #[serde_as(as = "DisplayFromStr")]
    pub executed_buy_amount: BigDecimal,
    pub status: Status,
}

/// Lifecycle of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Open,
    Fulfilled,
    Cancelled,
    Expired,
}

impl Order {
    /// Assemble the wire order from its row at the given time.
    pub fn new(row: OrderRow, now_unix: i64) -> Self {
        let status = status(&row, now_unix);
        Self {
            uid: format!("0x{}", const_hex::encode(row.uid.0)),
            owner: Pubkey::new_from_array(row.owner.0),
            sell_token: Pubkey::new_from_array(row.sell_token.0),
            buy_token: Pubkey::new_from_array(row.buy_token.0),
            sell_token_account: Pubkey::new_from_array(row.sell_token_account.0),
            buy_token_account: Pubkey::new_from_array(row.buy_token_account.0),
            sell_amount: row.sell_amount,
            buy_amount: row.buy_amount,
            valid_to: row.valid_to,
            kind: row.kind,
            partially_fillable: row.partially_fillable,
            app_data: format!("0x{}", const_hex::encode(row.app_data.0)),
            order_pda: Pubkey::new_from_array(row.order_pda.0),
            creation_date: row.creation_timestamp,
            executed_sell_amount: row.amount_withdrawn,
            executed_buy_amount: row.amount_received,
            status,
        }
    }
}

/// A full fill on the order's own side wins, then cancellation, then expiry.
/// Fulfilled must beat cancelled: reclaiming a filled order's PDA to recover
/// rent stamps a cancellation timestamp, and that cleanup does not undo the
/// fill.
fn status(row: &OrderRow, now_unix: i64) -> Status {
    let filled = match row.kind.as_str() {
        "sell" => row.amount_withdrawn >= row.sell_amount,
        "buy" => row.amount_received >= row.buy_amount,
        // The DB enum only holds sell and buy, a new variant must fail loud.
        other => unreachable!("unknown order kind {other}"),
    };
    if filled {
        return Status::Fulfilled;
    }
    if row.cancellation_timestamp.is_some() {
        return Status::Cancelled;
    }
    if row.valid_to < now_unix {
        return Status::Expired;
    }
    Status::Open
}

#[cfg(test)]
mod tests {
    use {super::*, database::byte_array::ByteArray};

    fn row() -> OrderRow {
        OrderRow {
            uid: ByteArray([0x11; 32]),
            owner: ByteArray([0x22; 32]),
            sell_token: ByteArray([0x33; 32]),
            buy_token: ByteArray([0x44; 32]),
            sell_token_account: ByteArray([0x55; 32]),
            buy_token_account: ByteArray([0x66; 32]),
            sell_amount: 1_000.into(),
            buy_amount: 500.into(),
            valid_to: 2_000,
            kind: "sell".to_string(),
            partially_fillable: false,
            app_data: ByteArray([0x77; 32]),
            creation_timestamp: DateTime::from_timestamp(1_000, 0).unwrap(),
            order_pda: ByteArray([0x88; 32]),
            amount_withdrawn: 0.into(),
            amount_received: 0.into(),
            cancellation_timestamp: None,
        }
    }

    #[test]
    fn status_precedence() {
        assert_eq!(status(&row(), 1_500), Status::Open);
        assert_eq!(status(&row(), 2_001), Status::Expired);

        let filled = OrderRow {
            amount_withdrawn: 1_000.into(),
            ..row()
        };
        // A full fill outranks expiry.
        assert_eq!(status(&filled, 2_001), Status::Fulfilled);

        let filled_buy = OrderRow {
            kind: "buy".to_string(),
            amount_received: 500.into(),
            ..row()
        };
        assert_eq!(status(&filled_buy, 1_500), Status::Fulfilled);

        let reclaimed_after_fill = OrderRow {
            cancellation_timestamp: Some(DateTime::from_timestamp(1_100, 0).unwrap()),
            amount_withdrawn: 1_000.into(),
            ..row()
        };
        // Reclaiming a filled order's PDA is cleanup, not a cancellation.
        assert_eq!(status(&reclaimed_after_fill, 1_500), Status::Fulfilled);

        let cancelled = OrderRow {
            cancellation_timestamp: Some(DateTime::from_timestamp(1_100, 0).unwrap()),
            ..row()
        };
        assert_eq!(status(&cancelled, 1_500), Status::Cancelled);
    }

    #[test]
    fn wire_format_is_stable() {
        let order = Order::new(row(), 1_500);
        let json = serde_json::to_value(&order).unwrap();
        assert_eq!(json["uid"], format!("0x{}", "11".repeat(32)));
        assert_eq!(json["sellAmount"], "1000");
        assert_eq!(json["executedSellAmount"], "0");
        assert_eq!(json["status"], "open");
        assert_eq!(json["kind"], "sell");
        assert_eq!(json["appData"], format!("0x{}", "77".repeat(32)));
        assert_eq!(json["creationDate"], "1970-01-01T00:16:40Z");
    }
}

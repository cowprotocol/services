//! The Solana auction domain: solvable orders typed over the shared chain
//! vocabulary and their assembly from database rows.

use {
    crate::{db, run_loop::AuctionInfo},
    anyhow::{Context, Result, bail},
    bigdecimal::{BigDecimal, ToPrimitive},
    chain_types::solana::{IntentHash, Pubkey},
    sqlx::PgExecutor,
};

/// Whether the order sells an exact amount or buys an exact amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderKind {
    Sell,
    Buy,
}

/// One solvable order.
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub uid: IntentHash,
    pub owner: Pubkey,
    pub sell_token: Pubkey,
    pub buy_token: Pubkey,
    pub sell_token_account: Pubkey,
    pub buy_token_account: Pubkey,
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub order_pda: Pubkey,
}

/// The cut auction the loop fans out to solvers.
#[derive(Clone, Debug)]
pub struct Auction {
    /// Autopilot-assigned id. Excluded from equality: the dedupe compares two
    /// cuts by content, and the id is allocated only for a fresh cut.
    pub id: i64,
    pub orders: Vec<Order>,
}

impl PartialEq for Auction {
    fn eq(&self, other: &Self) -> bool {
        self.orders == other.orders
    }
}

impl AuctionInfo for Auction {
    fn id(&self) -> i64 {
        self.id
    }
}

/// Cut an auction from the orders currently open for solving.
pub async fn cut(ex: impl PgExecutor<'_>, id: i64, now_unix: i64) -> Result<Auction> {
    let orders = orders_from_rows(db::open_orders(ex, now_unix).await?);
    Ok(Auction { id, orders })
}

/// A row the indexer wrote always converts (on-chain values fit the domain
/// types), so a failure means corrupt data. The corrupt order is skipped
/// instead of failing the cut, which would block solving for every other
/// order.
fn orders_from_rows(rows: Vec<db::OrderRow>) -> Vec<Order> {
    rows.into_iter()
        .filter_map(|row| {
            let uid = row.uid;
            Order::from_row(row)
                .map_err(|err| {
                    tracing::warn!(uid = %hex::encode(uid.0), ?err, "skipping corrupt order row")
                })
                .ok()
        })
        .collect()
}

impl Order {
    fn from_row(row: db::OrderRow) -> Result<Self> {
        Ok(Self {
            uid: IntentHash(row.uid.0),
            owner: Pubkey(row.owner.0),
            sell_token: Pubkey(row.sell_token.0),
            buy_token: Pubkey(row.buy_token.0),
            sell_token_account: Pubkey(row.sell_token_account.0),
            buy_token_account: Pubkey(row.buy_token_account.0),
            sell_amount: to_amount(&row.sell_amount).context("sell_amount")?,
            buy_amount: to_amount(&row.buy_amount).context("buy_amount")?,
            valid_to: row.valid_to.try_into().context("valid_to")?,
            kind: match row.kind.as_str() {
                "sell" => OrderKind::Sell,
                "buy" => OrderKind::Buy,
                other => bail!("unknown order kind {other:?}"),
            },
            partially_fillable: row.partially_fillable,
            order_pda: Pubkey(row.order_pda.0),
        })
    }
}

/// Token amounts are `numeric(78,0)` in the database but u64 on chain.
fn to_amount(value: &BigDecimal) -> Result<u64> {
    value
        .to_u64()
        .with_context(|| format!("amount {value} does not fit u64"))
}

#[cfg(test)]
mod tests {
    use {
        super::{Auction, Order, OrderKind},
        crate::db::OrderRow,
        bigdecimal::BigDecimal,
        database::byte_array::ByteArray,
    };

    fn row() -> OrderRow {
        OrderRow {
            uid: ByteArray([1; 32]),
            owner: ByteArray([2; 32]),
            sell_token: ByteArray([3; 32]),
            buy_token: ByteArray([4; 32]),
            sell_token_account: ByteArray([5; 32]),
            buy_token_account: ByteArray([6; 32]),
            sell_amount: BigDecimal::from(u64::MAX),
            buy_amount: BigDecimal::from(1_000u64),
            valid_to: 42,
            kind: "sell".to_owned(),
            partially_fillable: false,
            order_pda: ByteArray([7; 32]),
        }
    }

    #[test]
    fn converts_a_row_and_rejects_out_of_range_values() {
        let order = Order::from_row(row()).unwrap();
        assert_eq!(order.sell_amount, u64::MAX);
        assert_eq!(order.kind, OrderKind::Sell);

        let mut too_big = row();
        too_big.sell_amount = BigDecimal::from(u64::MAX) + BigDecimal::from(1u64);
        assert!(Order::from_row(too_big).is_err());

        let mut bad_kind = row();
        bad_kind.kind = "liquidity".to_owned();
        assert!(Order::from_row(bad_kind).is_err());
    }

    #[test]
    fn a_corrupt_row_is_skipped_not_fatal() {
        let mut corrupt = row();
        corrupt.sell_amount = BigDecimal::from(u64::MAX) + BigDecimal::from(1u64);
        let orders = super::orders_from_rows(vec![row(), corrupt]);
        assert_eq!(orders.len(), 1);
    }

    #[test]
    fn auction_equality_ignores_the_id() {
        let orders = vec![Order::from_row(row()).unwrap()];
        let a = Auction {
            id: 1,
            orders: orders.clone(),
        };
        let b = Auction { id: 2, orders };
        assert_eq!(a, b);
        assert_ne!(
            a,
            Auction {
                id: 1,
                orders: vec![],
            }
        );
    }
}

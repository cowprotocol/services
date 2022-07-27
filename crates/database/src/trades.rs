use crate::{Address, OrderUid, TransactionHash};
use bigdecimal::BigDecimal;
use futures::stream::BoxStream;
use sqlx::PgConnection;

#[derive(Clone, Debug, Default, PartialEq, sqlx::FromRow)]
pub struct TradesQueryRow {
    pub block_number: i64,
    pub log_index: i64,
    pub order_uid: OrderUid,
    pub buy_amount: BigDecimal,
    pub sell_amount: BigDecimal,
    pub sell_amount_before_fees: BigDecimal,
    pub owner: Address,
    pub buy_token: Address,
    pub sell_token: Address,
    pub tx_hash: Option<TransactionHash>,
}

pub fn trades<'a>(
    ex: &'a mut PgConnection,
    owner_filter: Option<&'a Address>,
    order_uid_filter: Option<&'a OrderUid>,
) -> BoxStream<'a, Result<TradesQueryRow, sqlx::Error>> {
    const QUERY: &str = r#"
SELECT
    t.block_number,
    t.log_index,
    t.order_uid,
    t.buy_amount,
    t.sell_amount,
    t.sell_amount - t.fee_amount as sell_amount_before_fees,
    o.owner,
    o.buy_token,
    o.sell_token,
    settlement.tx_hash
FROM trades t
LEFT OUTER JOIN LATERAL (
    SELECT tx_hash FROM settlements s
    WHERE s.block_number = t.block_number
    AND   s.log_index > t.log_index
    ORDER BY s.log_index ASC
    LIMIT 1
) AS settlement ON true
JOIN orders o
ON o.uid = t.order_uid
WHERE
    o.uid IS NOT null
AND
    ($1 IS NULL OR o.owner = $1)
AND
    ($2 IS NULL OR o.uid = $2)
    "#;

    sqlx::query_as(QUERY)
        .bind(owner_filter)
        .bind(order_uid_filter)
        .fetch(ex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        byte_array::ByteArray,
        events::{Event, EventIndex, Settlement, Trade},
        orders::Order,
        PgTransaction,
    };
    use futures::TryStreamExt;
    use sqlx::Connection;

    async fn generate_owners_and_order_ids(
        num_owners: usize,
        num_orders: usize,
    ) -> (Vec<Address>, Vec<OrderUid>) {
        let owners: Vec<Address> = (0..num_owners).map(|t| ByteArray([t as u8; 20])).collect();
        let order_ids: Vec<OrderUid> = (0..num_orders).map(|i| ByteArray([i as u8; 56])).collect();
        (owners, order_ids)
    }

    async fn add_trade(
        ex: &mut PgTransaction<'_>,
        owner: Address,
        order_uid: OrderUid,
        event_index: EventIndex,
        tx_hash: Option<TransactionHash>,
    ) -> TradesQueryRow {
        crate::events::append(
            ex,
            &[(
                event_index,
                Event::Trade(Trade {
                    order_uid: ByteArray(order_uid.0),
                    ..Default::default()
                }),
            )],
        )
        .await
        .unwrap();
        TradesQueryRow {
            block_number: event_index.block_number,
            log_index: event_index.log_index,
            order_uid,
            owner,
            tx_hash,
            ..Default::default()
        }
    }

    async fn add_order_and_trade(
        ex: &mut PgTransaction<'_>,
        owner: Address,
        order_uid: OrderUid,
        event_index: EventIndex,
        tx_hash: Option<TransactionHash>,
    ) -> TradesQueryRow {
        let order = Order {
            uid: order_uid,
            owner,
            ..Default::default()
        };
        crate::orders::insert_order(ex, &order).await.unwrap();
        add_trade(ex, owner, order_uid, event_index, tx_hash).await
    }

    async fn assert_trades(
        db: &mut PgConnection,
        owner_filter: Option<&Address>,
        order_uid_filter: Option<&OrderUid>,
        expected: &[TradesQueryRow],
    ) {
        let filtered = trades(db, owner_filter, order_uid_filter)
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        assert_eq!(filtered, expected);
    }

    // Testing trades without corresponding settlement events
    #[tokio::test]
    #[ignore]
    async fn postgres_trades_without_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;
        let event_index_a = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_a =
            add_order_and_trade(&mut db, owners[0], order_ids[0], event_index_a, None).await;
        assert_trades(&mut db, None, None, &[trade_a.clone()]).await;

        let event_index_b = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let trade_b =
            add_order_and_trade(&mut db, owners[0], order_ids[1], event_index_b, None).await;
        assert_trades(&mut db, None, None, &[trade_a, trade_b]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_owner_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(3, 2).await;

        let event_index_0 = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_0 =
            add_order_and_trade(&mut db, owners[0], order_ids[0], event_index_0, None).await;

        let event_index_1 = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let trade_1 =
            add_order_and_trade(&mut db, owners[1], order_ids[1], event_index_1, None).await;

        assert_trades(&mut db, Some(&owners[0]), None, &[trade_0]).await;
        assert_trades(&mut db, Some(&owners[1]), None, &[trade_1]).await;
        assert_trades(&mut db, Some(&owners[2]), None, &[]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_order_uid_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 3).await;

        let event_index_0 = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_0 =
            add_order_and_trade(&mut db, owners[0], order_ids[0], event_index_0, None).await;

        let event_index_1 = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let trade_1 =
            add_order_and_trade(&mut db, owners[1], order_ids[1], event_index_1, None).await;

        assert_trades(&mut db, None, Some(&order_ids[0]), &[trade_0]).await;
        assert_trades(&mut db, None, Some(&order_ids[1]), &[trade_1]).await;
        assert_trades(&mut db, None, Some(&order_ids[2]), &[]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trade_without_matching_order() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(1, 1).await;

        let event_index = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        add_trade(&mut db, owners[0], order_ids[0], event_index, None).await;
        // Trade exists in DB but no matching order
        assert_trades(&mut db, None, Some(&order_ids[0]), &[]).await;
        assert_trades(&mut db, Some(&owners[0]), None, &[]).await;
    }

    // Testing Trades with settlements
    async fn add_settlement(
        ex: &mut PgTransaction<'_>,
        event_index: EventIndex,
        solver: Address,
        transaction_hash: TransactionHash,
    ) -> Settlement {
        crate::events::append(
            ex,
            &[(
                event_index,
                Event::Settlement(Settlement {
                    solver,
                    transaction_hash,
                }),
            )],
        )
        .await
        .unwrap();
        Settlement {
            solver,
            transaction_hash,
        }
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_having_same_settlement_with_and_without_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            Default::default(),
            Default::default(),
        )
        .await;

        let trade_a = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a.clone()]).await;

        let trade_b = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a, trade_b]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_same_settlement_no_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            Default::default(),
            Default::default(),
        )
        .await;

        add_trade(
            &mut db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement.transaction_hash),
        )
        .await;

        add_trade(
            &mut db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        // Trades query returns nothing when there are no corresponding orders.
        assert_trades(&mut db, None, None, &[]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_two_settlements_in_same_block() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement_a = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Default::default(),
            Default::default(),
        )
        .await;
        let settlement_b = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 3,
            },
            Default::default(),
            ByteArray([2; 32]),
        )
        .await;

        let trade_a = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement_a.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a.clone()]).await;

        let trade_b = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 2,
            },
            Some(settlement_b.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a, trade_b]).await;
    }
}

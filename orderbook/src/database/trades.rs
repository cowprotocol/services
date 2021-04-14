use crate::conversions::{big_decimal_to_big_uint, h160_from_vec, h256_from_vec};
use crate::database::Database;
use anyhow::{anyhow, Context, Result};
use bigdecimal::BigDecimal;
use ethcontract::H160;
use futures::{stream::TryStreamExt, Stream};
use model::order::OrderUid;
use model::trade::Trade;
use std::convert::TryInto;

/// Any default value means that this field is unfiltered.
#[derive(Debug, Default, PartialEq)]
pub struct TradeFilter {
    pub owner: Option<H160>,
    pub order_uid: Option<OrderUid>,
}

impl Database {
    pub fn trades<'a>(&'a self, filter: &'a TradeFilter) -> impl Stream<Item = Result<Trade>> + 'a {
        const QUERY: &str = "\
            SELECT \
                t.block_number, \
                t.log_index, \
                t.order_uid, \
                t.buy_amount, \
                t.sell_amount, \
                t.sell_amount - t.fee_amount as sell_amount_before_fees,\
                o.owner, \
                o.buy_token, \
                o.sell_token, \
                settlement.tx_hash \
            FROM trades t \
            LEFT OUTER JOIN LATERAL ( \
                SELECT tx_hash FROM settlements s \
                WHERE s.block_number = t.block_number \
                AND   s.log_index > t.log_index \
                ORDER BY s.log_index ASC \
                LIMIT 1 \
            ) AS settlement ON true \
            JOIN orders o \
            ON o.uid = t.order_uid \
            WHERE \
                o.uid IS NOT null \
            AND \
                ($1 IS NULL OR o.owner = $1) \
            AND \
                ($2 IS NULL OR o.uid = $2);";

        sqlx::query_as(QUERY)
            .bind(filter.owner.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.order_uid.as_ref().map(|uid| uid.0.as_ref()))
            .fetch(&self.pool)
            .err_into()
            .and_then(|row: TradesQueryRow| async move { row.into_trade() })
    }
}

#[derive(sqlx::FromRow)]
struct TradesQueryRow {
    block_number: i64,
    log_index: i64,
    order_uid: Vec<u8>,
    buy_amount: BigDecimal,
    sell_amount: BigDecimal,
    sell_amount_before_fees: BigDecimal,
    owner: Vec<u8>,
    buy_token: Vec<u8>,
    sell_token: Vec<u8>,
    tx_hash: Option<Vec<u8>>,
}

impl TradesQueryRow {
    fn into_trade(self) -> Result<Trade> {
        let block_number = self
            .block_number
            .try_into()
            .context("block_number is not u32")?;
        let log_index = self.log_index.try_into().context("log_index is not u32")?;
        let order_uid = OrderUid(
            self.order_uid
                .try_into()
                .map_err(|_| anyhow!("order uid has wrong length"))?,
        );
        let buy_amount = big_decimal_to_big_uint(&self.buy_amount)
            .ok_or_else(|| anyhow!("buy_amount is not an unsigned integer"))?;
        let sell_amount = big_decimal_to_big_uint(&self.sell_amount)
            .ok_or_else(|| anyhow!("sell_amount is not an unsigned integer"))?;
        let sell_amount_before_fees = big_decimal_to_big_uint(&self.sell_amount_before_fees)
            .ok_or_else(|| anyhow!("sell_amount_before_fees is not an unsigned integer"))?;
        let owner = h160_from_vec(self.owner)?;
        let buy_token = h160_from_vec(self.buy_token)?;
        let sell_token = h160_from_vec(self.sell_token)?;
        let tx_hash = self.tx_hash.map(h256_from_vec).transpose()?;
        Ok(Trade {
            block_number,
            log_index,
            order_uid,
            buy_amount,
            sell_amount,
            sell_amount_before_fees,
            owner,
            buy_token,
            sell_token,
            tx_hash,
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::database::{Event, EventIndex, Settlement as DbSettlement, Trade as DbTrade};
    use ethcontract::H256;
    use model::order::{Order, OrderCreation, OrderMetaData};
    use model::trade::Trade;
    use std::collections::HashSet;

    async fn generate_owners_and_order_ids(
        num_owners: usize,
        num_orders: usize,
    ) -> (Vec<H160>, Vec<OrderUid>) {
        let owners: Vec<H160> = (0..num_owners)
            .map(|t| H160::from_low_u64_be(t as u64))
            .collect();
        let order_ids: Vec<OrderUid> = (0..num_orders).map(|i| OrderUid([i as u8; 56])).collect();
        return (owners, order_ids);
    }

    async fn add_trade(
        db: &Database,
        owner: H160,
        order_uid: OrderUid,
        event_index: EventIndex,
        tx_hash: Option<H256>,
    ) -> Trade {
        let trade = Trade {
            block_number: event_index.block_number,
            log_index: event_index.log_index,
            tx_hash,
            order_uid,
            owner,
            ..Default::default()
        };
        db.insert_events(vec![(
            event_index,
            Event::Trade(DbTrade {
                order_uid,
                ..Default::default()
            }),
        )])
        .await
        .unwrap();
        trade
    }

    async fn add_order_and_trade(
        db: &Database,
        owner: H160,
        order_uid: OrderUid,
        event_index: EventIndex,
        tx_hash: Option<H256>,
    ) -> Trade {
        let order = Order {
            order_meta_data: OrderMetaData {
                owner,
                uid: order_uid,
                ..Default::default()
            },
            order_creation: OrderCreation {
                ..Default::default()
            },
        };
        db.insert_order(&order).await.unwrap();
        add_trade(db, owner, order_uid, event_index, tx_hash).await
    }

    async fn assert_trades(db: &Database, filter: &TradeFilter, expected: &[Trade]) {
        let filtered = db
            .trades(&filter)
            .try_collect::<HashSet<Trade>>()
            .await
            .unwrap();
        let expected = expected.iter().cloned().collect::<HashSet<_>>();
        assert_eq!(filtered, expected);
    }

    // Testing trades without corresponding settlement events
    #[tokio::test]
    #[ignore]
    async fn postgres_trades_without_filter() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&db, &TradeFilter::default(), &[]).await;
        let event_index_a = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_a = add_order_and_trade(&db, owners[0], order_ids[0], event_index_a, None).await;
        assert_trades(&db, &TradeFilter::default(), &[trade_a.clone()]).await;

        let event_index_b = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let trade_b = add_order_and_trade(&db, owners[0], order_ids[1], event_index_b, None).await;
        assert_trades(&db, &TradeFilter::default(), &[trade_a, trade_b]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_owner_filter() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let (owners, order_ids) = generate_owners_and_order_ids(3, 2).await;

        let event_index_0 = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_0 = add_order_and_trade(&db, owners[0], order_ids[0], event_index_0, None).await;

        let event_index_1 = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let trade_1 = add_order_and_trade(&db, owners[1], order_ids[1], event_index_1, None).await;

        assert_trades(
            &db,
            &TradeFilter {
                owner: Some(owners[0]),
                ..Default::default()
            },
            &[trade_0],
        )
        .await;

        assert_trades(
            &db,
            &TradeFilter {
                owner: Some(owners[1]),
                ..Default::default()
            },
            &[trade_1],
        )
        .await;

        assert_trades(
            &db,
            &TradeFilter {
                owner: Some(owners[2]),
                ..Default::default()
            },
            &[],
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_order_uid_filter() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 3).await;

        let event_index_0 = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_0 = add_order_and_trade(&db, owners[0], order_ids[0], event_index_0, None).await;

        let event_index_1 = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let trade_1 = add_order_and_trade(&db, owners[1], order_ids[1], event_index_1, None).await;

        assert_trades(
            &db,
            &TradeFilter {
                order_uid: Some(order_ids[0]),
                ..Default::default()
            },
            &[trade_0],
        )
        .await;

        assert_trades(
            &db,
            &TradeFilter {
                order_uid: Some(order_ids[1]),
                ..Default::default()
            },
            &[trade_1],
        )
        .await;

        assert_trades(
            &db,
            &TradeFilter {
                order_uid: Some(order_ids[2]),
                ..Default::default()
            },
            &[],
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trade_without_matching_order() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(1, 1).await;

        let event_index = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        add_trade(&db, owners[0], order_ids[0], event_index, None).await;
        // Trade exists in DB but no matching order
        assert_trades(
            &db,
            &TradeFilter {
                order_uid: Some(order_ids[0]),
                ..Default::default()
            },
            &[],
        )
        .await;

        assert_trades(
            &db,
            &TradeFilter {
                owner: Some(owners[0]),
                ..Default::default()
            },
            &[],
        )
        .await;
    }

    // Testing Trades with settlements
    async fn add_settlement(
        db: &Database,
        event_index: EventIndex,
        solver: H160,
        transaction_hash: H256,
    ) -> DbSettlement {
        db.insert_events(vec![(
            event_index,
            Event::Settlement(DbSettlement {
                solver,
                transaction_hash,
            }),
        )])
        .await
        .unwrap();
        DbSettlement {
            solver,
            transaction_hash,
        }
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_having_same_settlement_with_and_without_orders() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&db, &TradeFilter::default(), &[]).await;

        let settlement = add_settlement(
            &db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            H160::default(),
            H256::from_low_u64_be(1),
        )
        .await;

        let trade_a = add_order_and_trade(
            &db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        assert_trades(&db, &TradeFilter::default(), &[trade_a.clone()]).await;

        let trade_b = add_order_and_trade(
            &db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        assert_trades(&db, &TradeFilter::default(), &[trade_a, trade_b]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_same_settlement_no_orders() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&db, &TradeFilter::default(), &[]).await;

        let settlement = add_settlement(
            &db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            H160::default(),
            H256::from_low_u64_be(1),
        )
        .await;

        add_trade(
            &db,
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
            &db,
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
        assert_trades(&db, &TradeFilter::default(), &[]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_two_settlements_in_same_block() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&db, &TradeFilter::default(), &[]).await;

        let settlement_a = add_settlement(
            &db,
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            H160::default(),
            H256::from_low_u64_be(1),
        )
        .await;
        let settlement_b = add_settlement(
            &db,
            EventIndex {
                block_number: 0,
                log_index: 3,
            },
            H160::default(),
            H256::from_low_u64_be(2),
        )
        .await;

        let trade_a = add_order_and_trade(
            &db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement_a.transaction_hash),
        )
        .await;
        assert_trades(&db, &TradeFilter::default(), &[trade_a.clone()]).await;

        let trade_b = add_order_and_trade(
            &db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 2,
            },
            Some(settlement_b.transaction_hash),
        )
        .await;
        assert_trades(&db, &TradeFilter::default(), &[trade_a, trade_b]).await;
    }
}

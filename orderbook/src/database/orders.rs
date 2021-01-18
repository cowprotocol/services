use super::Database;
use crate::integer_conversions::*;
use anyhow::{anyhow, Context, Result};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use futures::{stream::TryStreamExt, Stream};
use model::{
    order::{Order, OrderCreation, OrderKind, OrderMetaData, OrderUid},
    Signature,
};
use primitive_types::H160;
use std::convert::TryInto;

/// Any default value means that this field is unfiltered.
#[derive(Default)]
pub struct OrderFilter<'a> {
    pub min_valid_to: u32,
    pub owner: Option<&'a H160>,
    pub sell_token: Option<&'a H160>,
    pub buy_token: Option<&'a H160>,
    pub exclude_fully_executed: bool,
    pub exclude_invalidated: bool,
}

#[derive(sqlx::Type)]
#[sqlx(rename = "OrderKind")]
#[sqlx(rename_all = "lowercase")]
enum DbOrderKind {
    Buy,
    Sell,
}

impl DbOrderKind {
    fn from(order_kind: OrderKind) -> Self {
        match order_kind {
            OrderKind::Buy => Self::Buy,
            OrderKind::Sell => Self::Sell,
        }
    }

    fn into(self) -> OrderKind {
        match self {
            Self::Buy => OrderKind::Buy,
            Self::Sell => OrderKind::Sell,
        }
    }
}

impl Database {
    // TODO: Errors if order uid already exists. We might want to have different behavior like
    // indicating this in the return value or simply allowing it to happen.
    pub async fn insert_order(&self, order: &Order) -> Result<()> {
        const QUERY: &str = "\
            INSERT INTO orders (
                uid, owner, creation_timestamp, sell_token, buy_token, sell_amount, buy_amount, \
                valid_to, app_data, fee_amount, kind, partially_fillable, signature) \
            VALUES ( \
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13);";
        sqlx::query(QUERY)
            .bind(order.order_meta_data.uid.0.as_ref())
            .bind(order.order_meta_data.owner.as_bytes())
            .bind(order.order_meta_data.creation_date)
            .bind(order.order_creation.sell_token.as_bytes())
            .bind(order.order_creation.buy_token.as_bytes())
            .bind(u256_to_big_decimal(&order.order_creation.sell_amount))
            .bind(u256_to_big_decimal(&order.order_creation.buy_amount))
            .bind(order.order_creation.valid_to)
            .bind(order.order_creation.app_data)
            .bind(u256_to_big_decimal(&order.order_creation.fee_amount))
            .bind(DbOrderKind::from(order.order_creation.kind))
            .bind(order.order_creation.partially_fillable)
            .bind(order.order_creation.signature.to_bytes().as_ref())
            .execute(&self.pool)
            .await
            .context("insert_order failed")
            .map(|_| ())
    }

    pub fn orders<'a>(&'a self, filter: &'a OrderFilter) -> impl Stream<Item = Result<Order>> + 'a {
        // The `or`s in the `where` clause are there so that each filter is ignored when not set.
        // We use a subquery instead of a `having` clause in the inner query because we would not be
        // able to use the `sum_*` columns there.
        const QUERY: &str = "\
        SELECT * FROM ( \
            SELECT \
                o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, \
                o.buy_amount, o.valid_to, o.app_data, o.fee_amount, o.kind, o.partially_fillable, \
                o.signature, \
                COALESCE(SUM(t.buy_amount), 0) AS sum_buy, \
                COALESCE(SUM(t.sell_amount), 0) AS sum_sell, \
                COALESCE(SUM(t.fee_amount), 0) AS sum_fee, \
                COUNT(invalidations.*) > 0 AS invalidated \
            FROM \
                orders o \
                LEFT OUTER JOIN trades t ON o.uid = t.order_uid \
                LEFT OUTER JOIN invalidations ON o.uid = invalidations.order_uid \
            WHERE \
                o.valid_to >= $1 AND \
                ($2 IS NULL OR o.owner = $2) AND \
                ($3 IS NULL OR o.sell_token = $3) AND \
                ($4 IS NULL OR o.buy_token = $4) \
            GROUP BY o.uid \
        ) AS unfiltered \
        WHERE
            ($5 OR CASE kind \
                WHEN 'sell' THEN sum_sell < sell_amount \
                WHEN 'buy' THEN sum_buy < buy_amount \
            END) AND \
            ($6 OR NOT invalidated);";

        sqlx::query_as(QUERY)
            .bind(filter.min_valid_to)
            .bind(filter.owner.map(|h160| h160.as_bytes()))
            .bind(filter.sell_token.map(|h160| h160.as_bytes()))
            .bind(filter.buy_token.map(|h160| h160.as_bytes()))
            .bind(!filter.exclude_fully_executed)
            .bind(!filter.exclude_invalidated)
            .fetch(&self.pool)
            .err_into()
            .and_then(|row: OrdersQueryRow| async move { row.into_order() })
    }
}

#[derive(sqlx::FromRow)]
struct OrdersQueryRow {
    uid: Vec<u8>,
    owner: Vec<u8>,
    creation_timestamp: DateTime<Utc>,
    sell_token: Vec<u8>,
    buy_token: Vec<u8>,
    sell_amount: BigDecimal,
    buy_amount: BigDecimal,
    valid_to: i64,
    app_data: i64,
    fee_amount: BigDecimal,
    kind: DbOrderKind,
    partially_fillable: bool,
    signature: Vec<u8>,
    sum_sell: BigDecimal,
    sum_buy: BigDecimal,
    sum_fee: BigDecimal,
    invalidated: bool,
}

fn h160_from_vec(vec: Vec<u8>) -> Result<H160> {
    let array: [u8; 20] = vec
        .try_into()
        .map_err(|_| anyhow!("h160 has wrong length"))?;
    Ok(H160::from(array))
}

impl OrdersQueryRow {
    fn into_order(self) -> Result<Order> {
        let order_meta_data = OrderMetaData {
            creation_date: self.creation_timestamp,
            owner: h160_from_vec(self.owner)?,
            uid: OrderUid(
                self.uid
                    .try_into()
                    .map_err(|_| anyhow!("order uid has wrong length"))?,
            ),
            executed_buy_amount: big_decimal_to_big_uint(&self.sum_buy)
                .ok_or_else(|| anyhow!("sum_buy is not an unsigned integer"))?,
            executed_sell_amount: big_decimal_to_big_uint(&self.sum_sell)
                .ok_or_else(|| anyhow!("sum_sell is not an unsigned integer"))?,
            executed_fee_amount: big_decimal_to_big_uint(&self.sum_fee)
                .ok_or_else(|| anyhow!("sum_fee is not an unsigned integer"))?,
            invalidated: self.invalidated,
        };
        let order_creation = OrderCreation {
            sell_token: h160_from_vec(self.sell_token)?,
            buy_token: h160_from_vec(self.buy_token)?,
            sell_amount: big_decimal_to_u256(&self.sell_amount)
                .ok_or_else(|| anyhow!("sell_amount is not U256"))?,
            buy_amount: big_decimal_to_u256(&self.buy_amount)
                .ok_or_else(|| anyhow!("buy_amount is not U256"))?,
            valid_to: self.valid_to.try_into().context("valid_to is not u32")?,
            app_data: self.app_data.try_into().context("app_data is not u32")?,
            fee_amount: big_decimal_to_u256(&self.fee_amount)
                .ok_or_else(|| anyhow!("buy_amount is not U256"))?,
            kind: self.kind.into(),
            partially_fillable: self.partially_fillable,
            signature: Signature::from_bytes(
                &self
                    .signature
                    .try_into()
                    .map_err(|_| anyhow!("signature has wrong length"))?,
            ),
        };
        Ok(Order {
            order_meta_data,
            order_creation,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::database::Trade;

    use super::*;
    use chrono::NaiveDateTime;
    use futures::StreamExt;
    use num_bigint::BigUint;
    use primitive_types::U256;
    use sqlx::Executor;
    use std::collections::HashSet;

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_same_order_twice_fails() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let order = Order::default();
        db.insert_order(&order).await.unwrap();
        assert!(db.insert_order(&order).await.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_order_roundtrip() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let filter = OrderFilter::default();
        assert!(db.orders(&filter).boxed().next().await.is_none());
        let order = Order {
            order_meta_data: OrderMetaData {
                creation_date: DateTime::<Utc>::from_utc(
                    NaiveDateTime::from_timestamp(1234567890, 0),
                    Utc,
                ),
                ..Default::default()
            },
            order_creation: OrderCreation {
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                sell_amount: 3.into(),
                buy_amount: U256::MAX,
                valid_to: u32::MAX,
                app_data: 4,
                fee_amount: 5.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                signature: Default::default(),
            },
        };
        db.insert_order(&order).await.unwrap();
        assert_eq!(
            db.orders(&filter)
                .try_collect::<Vec<Order>>()
                .await
                .unwrap(),
            vec![order]
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_filter_orders_by_address() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let orders = vec![
            Order {
                order_meta_data: OrderMetaData {
                    owner: H160::from_low_u64_be(0),
                    uid: OrderUid([0u8; 56]),
                    ..Default::default()
                },
                order_creation: OrderCreation {
                    sell_token: H160::from_low_u64_be(1),
                    buy_token: H160::from_low_u64_be(2),
                    valid_to: 10,
                    ..Default::default()
                },
            },
            Order {
                order_meta_data: OrderMetaData {
                    owner: H160::from_low_u64_be(0),
                    uid: OrderUid([1; 56]),
                    ..Default::default()
                },
                order_creation: OrderCreation {
                    sell_token: H160::from_low_u64_be(1),
                    buy_token: H160::from_low_u64_be(3),
                    valid_to: 11,
                    ..Default::default()
                },
            },
            Order {
                order_meta_data: OrderMetaData {
                    owner: H160::from_low_u64_be(2),
                    uid: OrderUid([2u8; 56]),
                    ..Default::default()
                },
                order_creation: OrderCreation {
                    sell_token: H160::from_low_u64_be(1),
                    buy_token: H160::from_low_u64_be(3),
                    valid_to: 12,
                    ..Default::default()
                },
            },
        ];
        for order in orders.iter() {
            db.insert_order(order).await.unwrap();
        }

        async fn assert_orders(db: &Database, filter: &OrderFilter<'_>, expected: &[Order]) {
            let filtered = db
                .orders(&filter)
                .try_collect::<HashSet<Order>>()
                .await
                .unwrap();
            let expected = expected.iter().cloned().collect::<HashSet<_>>();
            assert_eq!(filtered, expected);
        }

        let owner = H160::from_low_u64_be(0);
        assert_orders(
            &db,
            &OrderFilter {
                owner: Some(&owner),
                ..Default::default()
            },
            &orders[0..2],
        )
        .await;

        let sell_token = H160::from_low_u64_be(1);
        assert_orders(
            &db,
            &OrderFilter {
                sell_token: Some(&sell_token),
                ..Default::default()
            },
            &orders[0..3],
        )
        .await;

        let buy_token = H160::from_low_u64_be(3);
        assert_orders(
            &db,
            &OrderFilter {
                buy_token: Some(&buy_token),
                ..Default::default()
            },
            &orders[1..3],
        )
        .await;

        assert_orders(
            &db,
            &OrderFilter {
                min_valid_to: 10,
                ..Default::default()
            },
            &orders[0..3],
        )
        .await;

        assert_orders(
            &db,
            &OrderFilter {
                min_valid_to: 11,
                ..Default::default()
            },
            &orders[1..3],
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_filter_orders_by_fully_executed() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let order = Order {
            order_meta_data: Default::default(),
            order_creation: OrderCreation {
                kind: OrderKind::Sell,
                sell_amount: 10.into(),
                buy_amount: 100.into(),
                ..Default::default()
            },
        };
        db.insert_order(&order).await.unwrap();

        let get_order = |exclude_fully_executed| {
            let db = db.clone();
            async move {
                db.orders(&OrderFilter {
                    exclude_fully_executed,
                    ..Default::default()
                })
                .boxed()
                .next()
                .await
            }
        };

        let order = get_order(true).await.unwrap().unwrap();
        assert_eq!(
            order.order_meta_data.executed_sell_amount,
            BigUint::from(0u8)
        );

        db.insert_trades(vec![Trade {
            block_number: 0,
            log_index: 0,
            order_uid: order.order_meta_data.uid,
            sell_amount: 3.into(),
            ..Default::default()
        }])
        .await
        .unwrap();
        let order = get_order(true).await.unwrap().unwrap();
        assert_eq!(
            order.order_meta_data.executed_sell_amount,
            BigUint::from(3u8)
        );

        db.insert_trades(vec![Trade {
            block_number: 1,
            order_uid: order.order_meta_data.uid,
            sell_amount: 6.into(),
            ..Default::default()
        }])
        .await
        .unwrap();
        let order = get_order(true).await.unwrap().unwrap();
        assert_eq!(
            order.order_meta_data.executed_sell_amount,
            BigUint::from(9u8),
        );

        // The order disappears because it is fully executed.
        db.insert_trades(vec![Trade {
            block_number: 2,
            order_uid: order.order_meta_data.uid,
            sell_amount: 1.into(),
            ..Default::default()
        }])
        .await
        .unwrap();
        assert!(get_order(true).await.is_none());

        // If we include fully executed orders it is there.
        let order = get_order(false).await.unwrap().unwrap();
        assert_eq!(
            order.order_meta_data.executed_sell_amount,
            BigUint::from(10u8)
        );

        // Change order type and see that is returned as not fully executed again.
        let query = "UPDATE orders SET kind = 'buy';";
        db.pool.execute(query).await.unwrap();
        assert!(get_order(true).await.is_some());
    }

    // In the schema we set the type of executed amounts in individual events to a 78 decimal digit
    // number. Summing over multiple events could overflow this because the smart contract only
    // guarantees that the filled amount (which amount that is depends on order type) does not
    // overflow a U256. This test shows that postgres does not error if this happens because
    // inside the SUM the number can have more digits.
    #[tokio::test]
    #[ignore]
    async fn postgres_summed_executed_amount_does_not_overflow() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let order = Order {
            order_meta_data: Default::default(),
            order_creation: OrderCreation {
                kind: OrderKind::Sell,
                ..Default::default()
            },
        };
        db.insert_order(&order).await.unwrap();

        for i in 0..10 {
            db.insert_trades(vec![Trade {
                block_number: i,
                order_uid: order.order_meta_data.uid,
                sell_amount: U256::MAX,
                ..Default::default()
            }])
            .await
            .unwrap();
        }

        let order = db
            .orders(&OrderFilter::default())
            .boxed()
            .next()
            .await
            .unwrap()
            .unwrap();

        let expected = u256_to_big_uint(&U256::MAX) * BigUint::from(10u8);
        assert!(expected.to_string().len() > 78);
        assert_eq!(order.order_meta_data.executed_sell_amount, expected);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_filter_orders_by_invalidated() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let uid = OrderUid([0u8; 56]);
        let order = Order {
            order_meta_data: OrderMetaData {
                uid,
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&order).await.unwrap();

        let is_order_valid = || async {
            db.orders(&OrderFilter {
                exclude_invalidated: true,
                ..Default::default()
            })
            .boxed()
            .next()
            .await
            .transpose()
            .unwrap()
            .is_some()
        };

        assert!(is_order_valid().await);

        // Invalidating a different order doesn't affect first order.
        sqlx::query(
            "INSERT INTO invalidations (block_number, log_index, order_uid) VALUES ($1, $2, $3)",
        )
        .bind(0i64)
        .bind(0i64)
        .bind([1u8; 56].as_ref())
        .execute(&db.pool)
        .await
        .unwrap();
        assert!(is_order_valid().await);

        // But invalidating it does work
        sqlx::query(
            "INSERT INTO invalidations (block_number, log_index, order_uid) VALUES ($1, $2, $3)",
        )
        .bind(1i64)
        .bind(0i64)
        .bind([0u8; 56].as_ref())
        .execute(&db.pool)
        .await
        .unwrap();
        assert!(!is_order_valid().await);

        // And we can invalidate it several times.
        sqlx::query(
            "INSERT INTO invalidations (block_number, log_index, order_uid) VALUES ($1, $2, $3)",
        )
        .bind(2i64)
        .bind(0i64)
        .bind([0u8; 56].as_ref())
        .execute(&db.pool)
        .await
        .unwrap();
        assert!(!is_order_valid().await);
    }
}

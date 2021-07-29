use super::*;
use crate::conversions::*;
use anyhow::{anyhow, Context, Result};
use bigdecimal::{BigDecimal, Zero};
use chrono::{DateTime, Utc};
use futures::{stream::TryStreamExt, StreamExt};
use model::order::{BalanceFrom, BalanceTo};
use model::{
    order::{Order, OrderCreation, OrderKind, OrderMetaData, OrderStatus, OrderUid},
    Signature, SigningScheme,
};
use primitive_types::H160;
use std::{borrow::Cow, convert::TryInto};

#[async_trait::async_trait]
pub trait OrderStoring: Send + Sync {
    async fn insert_order(&self, order: &Order) -> Result<(), InsertionError>;
    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()>;
    fn orders<'a>(&'a self, filter: &'a OrderFilter) -> BoxStream<'a, Result<Order>>;
}

/// Any default value means that this field is unfiltered.
#[derive(Clone, Copy, Default)]
pub struct OrderFilter {
    pub min_valid_to: u32,
    pub owner: Option<H160>,
    pub sell_token: Option<H160>,
    pub buy_token: Option<H160>,
    pub exclude_fully_executed: bool,
    pub exclude_invalidated: bool,
    pub exclude_insufficient_balance: bool,
    pub exclude_unsupported_tokens: bool,
    pub uid: Option<OrderUid>,
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "OrderKind")]
#[sqlx(rename_all = "lowercase")]
pub enum DbOrderKind {
    Buy,
    Sell,
}

impl DbOrderKind {
    pub fn from(order_kind: OrderKind) -> Self {
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

/// Location for which the sellAmount should be drawn upon order fulfilment
#[derive(sqlx::Type)]
#[sqlx(type_name = "BalanceFrom")]
#[sqlx(rename_all = "snake_case")]
pub enum DbBalanceFrom {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
    /// Internal balances to the Vault with GPv2 relayer approval
    External,
}

impl DbBalanceFrom {
    pub fn from(order_kind: BalanceFrom) -> Self {
        match order_kind {
            BalanceFrom::Erc20 => Self::Erc20,
            BalanceFrom::Internal => Self::Internal,
            BalanceFrom::External => Self::External,
        }
    }
    fn into(self) -> BalanceFrom {
        match self {
            Self::Erc20 => BalanceFrom::Erc20,
            Self::Internal => BalanceFrom::Internal,
            Self::External => BalanceFrom::External,
        }
    }
}

/// Location for which the buyAmount should be transferred to order's receiver to upon fulfilment
#[derive(sqlx::Type)]
#[sqlx(type_name = "BalanceTo")]
#[sqlx(rename_all = "snake_case")]
pub enum DbBalanceTo {
    /// Pay trade proceeds as an ERC20 token transfer
    Erc20,
    /// Pay trade proceeds as a Vault internal balance transfer
    Internal,
}

impl DbBalanceTo {
    pub fn from(order_kind: BalanceTo) -> Self {
        match order_kind {
            BalanceTo::Erc20 => Self::Erc20,
            BalanceTo::Internal => Self::Internal,
        }
    }
    fn into(self) -> BalanceTo {
        match self {
            Self::Erc20 => BalanceTo::Erc20,
            Self::Internal => BalanceTo::Internal,
        }
    }
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "SigningScheme")]
#[sqlx(rename_all = "lowercase")]
pub enum DbSigningScheme {
    Eip712,
    EthSign,
}

impl DbSigningScheme {
    pub fn from(signing_scheme: SigningScheme) -> Self {
        match signing_scheme {
            SigningScheme::Eip712 => Self::Eip712,
            SigningScheme::EthSign => Self::EthSign,
        }
    }

    fn into(self) -> SigningScheme {
        match self {
            Self::Eip712 => SigningScheme::Eip712,
            Self::EthSign => SigningScheme::EthSign,
        }
    }
}

#[derive(Debug)]
pub enum InsertionError {
    DuplicatedRecord,
    DbError(sqlx::Error),
}

impl From<sqlx::Error> for InsertionError {
    fn from(err: sqlx::Error) -> Self {
        Self::DbError(err)
    }
}

#[async_trait::async_trait]
impl OrderStoring for Postgres {
    async fn insert_order(&self, order: &Order) -> Result<(), InsertionError> {
        const QUERY: &str = "\
            INSERT INTO orders (
                uid, owner, creation_timestamp, sell_token, buy_token, receiver, sell_amount, buy_amount, \
                valid_to, app_data, fee_amount, kind, partially_fillable, signature, signing_scheme, settlement_contract, balance_from, balance_to) \
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18);";
        let receiver = order
            .order_creation
            .receiver
            .map(|address| address.as_bytes().to_vec());
        sqlx::query(QUERY)
            .bind(order.order_meta_data.uid.0.as_ref())
            .bind(order.order_meta_data.owner.as_bytes())
            .bind(order.order_meta_data.creation_date)
            .bind(order.order_creation.sell_token.as_bytes())
            .bind(order.order_creation.buy_token.as_bytes())
            .bind(receiver)
            .bind(u256_to_big_decimal(&order.order_creation.sell_amount))
            .bind(u256_to_big_decimal(&order.order_creation.buy_amount))
            .bind(order.order_creation.valid_to)
            .bind(&order.order_creation.app_data[..])
            .bind(u256_to_big_decimal(&order.order_creation.fee_amount))
            .bind(DbOrderKind::from(order.order_creation.kind))
            .bind(order.order_creation.partially_fillable)
            .bind(order.order_creation.signature.to_bytes().as_ref())
            .bind(DbSigningScheme::from(order.order_creation.signing_scheme))
            .bind(order.order_meta_data.settlement_contract.as_bytes())
            .bind(DbBalanceFrom::from(order.order_creation.sell_token_balance))
            .bind(DbBalanceTo::from(order.order_creation.buy_token_balance))
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|err| {
                if let sqlx::Error::Database(db_err) = &err {
                    if let Some(Cow::Borrowed("23505")) = db_err.code() {
                        return InsertionError::DuplicatedRecord;
                    }
                }
                InsertionError::DbError(err)
            })
    }

    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()> {
        // We do not overwrite previously cancelled orders,
        // but this query does allow the user to soft cancel
        // an order that has already been invalidated on-chain.
        const QUERY: &str = "\
            UPDATE orders
            SET cancellation_timestamp = $1 \
            WHERE uid = $2\
            AND cancellation_timestamp IS NULL;";
        sqlx::query(QUERY)
            .bind(now)
            .bind(order_uid.0.as_ref())
            .execute(&self.pool)
            .await
            .context("cancel_order failed")
            .map(|_| ())
    }

    fn orders<'a>(&'a self, filter: &'a OrderFilter) -> BoxStream<'a, Result<Order>> {
        // The `or`s in the `where` clause are there so that each filter is ignored when not set.
        // We use a subquery instead of a `having` clause in the inner query because we would not be
        // able to use the `sum_*` columns there.
        const QUERY: &str = "\
        SELECT * FROM ( \
            SELECT \
                o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, \
                o.buy_amount, o.valid_to, o.app_data, o.fee_amount, o.kind, o.partially_fillable, \
                o.signature, o.receiver, o.signing_scheme, o.settlement_contract, o.balance_from, \
                o.balance_to, \
                COALESCE(SUM(t.buy_amount), 0) AS sum_buy, \
                COALESCE(SUM(t.sell_amount), 0) AS sum_sell, \
                COALESCE(SUM(t.fee_amount), 0) AS sum_fee, \
                (COUNT(invalidations.*) > 0 OR o.cancellation_timestamp IS NOT NULL) AS invalidated \
            FROM \
                orders o \
                LEFT OUTER JOIN trades t ON o.uid = t.order_uid \
                LEFT OUTER JOIN invalidations ON o.uid = invalidations.order_uid \
            WHERE \
                o.valid_to >= $1 AND \
                ($2 IS NULL OR o.owner = $2) AND \
                ($3 IS NULL OR o.sell_token = $3) AND \
                ($4 IS NULL OR o.buy_token = $4) AND \
                ($5 IS NULL OR o.uid = $5) \
            GROUP BY o.uid \
        ) AS unfiltered \
        WHERE
            ($6 OR CASE kind \
                WHEN 'sell' THEN sum_sell < sell_amount \
                WHEN 'buy' THEN sum_buy < buy_amount \
            END) AND \
            ($7 OR NOT invalidated);";

        sqlx::query_as(QUERY)
            .bind(filter.min_valid_to)
            .bind(filter.owner.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.sell_token.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.buy_token.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.uid.as_ref().map(|uid| uid.0.as_ref()))
            .bind(!filter.exclude_fully_executed)
            .bind(!filter.exclude_invalidated)
            .fetch(&self.pool)
            .err_into()
            .and_then(|row: OrdersQueryRow| async move { row.into_order() })
            .boxed()
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
    app_data: Vec<u8>,
    fee_amount: BigDecimal,
    kind: DbOrderKind,
    partially_fillable: bool,
    signature: Vec<u8>,
    sum_sell: BigDecimal,
    sum_buy: BigDecimal,
    sum_fee: BigDecimal,
    invalidated: bool,
    receiver: Option<Vec<u8>>,
    signing_scheme: DbSigningScheme,
    settlement_contract: Vec<u8>,
    balance_from: DbBalanceFrom,
    balance_to: DbBalanceTo,
}

impl OrdersQueryRow {
    fn calculate_status(&self) -> OrderStatus {
        match self.kind {
            DbOrderKind::Buy => {
                if is_buy_order_filled(&self.buy_amount, &self.sum_buy) {
                    return OrderStatus::Fulfilled;
                }
            }
            DbOrderKind::Sell => {
                if is_sell_order_filled(&self.sell_amount, &self.sum_sell, &self.sum_fee) {
                    return OrderStatus::Fulfilled;
                }
            }
        }
        if self.invalidated {
            return OrderStatus::Cancelled;
        }
        if self.valid_to < Utc::now().timestamp() {
            return OrderStatus::Expired;
        }
        OrderStatus::Open
    }

    fn into_order(self) -> Result<Order> {
        let status = self.calculate_status();

        let executed_sell_amount = big_decimal_to_big_uint(&self.sum_sell)
            .ok_or_else(|| anyhow!("sum_sell is not an unsigned integer"))?;
        let executed_fee_amount = big_decimal_to_big_uint(&self.sum_fee)
            .ok_or_else(|| anyhow!("sum_fee is not an unsigned integer"))?;
        let executed_sell_amount_before_fees = &executed_sell_amount - &executed_fee_amount;

        let order_meta_data = OrderMetaData {
            creation_date: self.creation_timestamp,
            owner: h160_from_vec(self.owner)?,
            uid: OrderUid(
                self.uid
                    .try_into()
                    .map_err(|_| anyhow!("order uid has wrong length"))?,
            ),
            available_balance: Default::default(),
            executed_buy_amount: big_decimal_to_big_uint(&self.sum_buy)
                .ok_or_else(|| anyhow!("sum_buy is not an unsigned integer"))?,
            executed_sell_amount,
            executed_sell_amount_before_fees,
            executed_fee_amount,
            invalidated: self.invalidated,
            status,
            settlement_contract: h160_from_vec(self.settlement_contract)?,
        };
        let order_creation = OrderCreation {
            sell_token: h160_from_vec(self.sell_token)?,
            buy_token: h160_from_vec(self.buy_token)?,
            receiver: self.receiver.map(h160_from_vec).transpose()?,
            sell_amount: big_decimal_to_u256(&self.sell_amount)
                .ok_or_else(|| anyhow!("sell_amount is not U256"))?,
            buy_amount: big_decimal_to_u256(&self.buy_amount)
                .ok_or_else(|| anyhow!("buy_amount is not U256"))?,
            valid_to: self.valid_to.try_into().context("valid_to is not u32")?,
            app_data: self
                .app_data
                .try_into()
                .map_err(|_| anyhow!("app_data is not [u8; 32]"))?,
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
            signing_scheme: self.signing_scheme.into(),
            sell_token_balance: self.balance_from.into(),
            buy_token_balance: self.balance_to.into(),
        };
        Ok(Order {
            order_meta_data,
            order_creation,
        })
    }
}

fn is_sell_order_filled(
    amount: &BigDecimal,
    executed_amount: &BigDecimal,
    executed_fee: &BigDecimal,
) -> bool {
    if executed_amount.is_zero() {
        return false;
    }
    let total_amount = executed_amount - executed_fee;
    total_amount == *amount
}

fn is_buy_order_filled(amount: &BigDecimal, executed_amount: &BigDecimal) -> bool {
    !executed_amount.is_zero() && *amount == *executed_amount
}

#[cfg(test)]
mod tests {
    use super::events::*;
    use super::*;
    use chrono::{Duration, NaiveDateTime};
    use futures::StreamExt;
    use num_bigint::BigUint;
    use primitive_types::U256;
    use shared::event_handling::EventIndex;
    use sqlx::Executor;
    use std::collections::HashSet;

    #[test]
    fn order_status() {
        let valid_to_timestamp = Utc::now() + Duration::days(1);

        // Open - sell (filled - 0%)
        let order_row = OrdersQueryRow {
            uid: vec![0; 56],
            owner: vec![0; 20],
            creation_timestamp: Utc::now(),
            sell_token: vec![1; 20],
            buy_token: vec![2; 20],
            sell_amount: BigDecimal::from(1),
            buy_amount: BigDecimal::from(1),
            valid_to: valid_to_timestamp.timestamp(),
            app_data: vec![0; 32],
            fee_amount: BigDecimal::default(),
            kind: DbOrderKind::Sell,
            partially_fillable: true,
            signature: vec![0; 65],
            receiver: None,
            sum_sell: BigDecimal::default(),
            sum_buy: BigDecimal::default(),
            sum_fee: BigDecimal::default(),
            invalidated: false,
            signing_scheme: DbSigningScheme::Eip712,
            settlement_contract: vec![0; 20],
            balance_from: DbBalanceFrom::External,
            balance_to: DbBalanceTo::Internal,
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Open);

        // Open - sell (almost filled - 99.99%)
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Sell,
            sell_amount: BigDecimal::from(10_000),
            sum_sell: BigDecimal::from(9_999),
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Open);

        // Filled - sell (filled - 100%)
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Sell,
            sell_amount: BigDecimal::from(2),
            sum_sell: BigDecimal::from(3),
            sum_fee: BigDecimal::from(1),
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Fulfilled);

        // Open - buy (filled - 0%)
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Buy,
            buy_amount: BigDecimal::from(1),
            sum_buy: BigDecimal::from(0),
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Open);

        // Open - buy (almost filled - 99.99%)
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Buy,
            buy_amount: BigDecimal::from(10_000),
            sum_buy: BigDecimal::from(9_999),
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Open);

        // Filled - buy (filled - 100%)
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Buy,
            buy_amount: BigDecimal::from(1),
            sum_buy: BigDecimal::from(1),
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Fulfilled);

        // Cancelled - no fills - sell
        let order_row = OrdersQueryRow {
            sum_sell: BigDecimal::default(),
            sum_buy: BigDecimal::default(),
            invalidated: true,
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Cancelled);

        // Cancelled - partial fill - sell
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Sell,
            sell_amount: BigDecimal::from(2),
            sum_sell: BigDecimal::from(1),
            sum_fee: BigDecimal::default(),
            invalidated: true,
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Cancelled);

        // Cancelled - partial fill - buy
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Buy,
            buy_amount: BigDecimal::from(2),
            sum_buy: BigDecimal::from(1),
            invalidated: true,
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Cancelled);

        // Expired - no fills
        let valid_to_yesterday = Utc::now() - Duration::days(1);

        let order_row = OrdersQueryRow {
            sum_sell: BigDecimal::default(),
            sum_buy: BigDecimal::default(),
            invalidated: false,
            valid_to: valid_to_yesterday.timestamp(),
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Expired);

        // Expired - partial fill - sell
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Sell,
            sell_amount: BigDecimal::from(2),
            sum_sell: BigDecimal::from(1),
            invalidated: false,
            valid_to: valid_to_yesterday.timestamp(),
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Expired);

        // Expired - partial fill - buy
        let order_row = OrdersQueryRow {
            kind: DbOrderKind::Buy,
            buy_amount: BigDecimal::from(2),
            sum_buy: BigDecimal::from(1),
            invalidated: false,
            valid_to: valid_to_yesterday.timestamp(),
            ..order_row
        };

        assert_eq!(order_row.calculate_status(), OrderStatus::Expired);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_same_order_twice_fails() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let order = Order::default();
        db.insert_order(&order).await.unwrap();
        match db.insert_order(&order).await {
            Err(InsertionError::DuplicatedRecord) => (),
            _ => panic!("Expecting DuplicatedRecord error"),
        };
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_order_roundtrip() {
        let db = Postgres::new("postgresql://").unwrap();
        for signing_scheme in &[SigningScheme::Eip712, SigningScheme::EthSign] {
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
                    receiver: Some(H160::from_low_u64_be(6)),
                    sell_amount: 3.into(),
                    buy_amount: U256::MAX,
                    valid_to: u32::MAX,
                    app_data: [4; 32],
                    fee_amount: 5.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: true,
                    signature: Default::default(),
                    signing_scheme: *signing_scheme,
                    sell_token_balance: BalanceFrom::Erc20,
                    buy_token_balance: BalanceTo::Internal,
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
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_cancel_order() {
        #[derive(sqlx::FromRow, Debug, PartialEq)]
        struct CancellationQueryRow {
            cancellation_timestamp: DateTime<Utc>,
        }

        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let filter = OrderFilter::default();
        assert!(db.orders(&filter).boxed().next().await.is_none());

        let order = Order::default();
        db.insert_order(&order).await.unwrap();
        let db_orders = db
            .orders(&filter)
            .try_collect::<Vec<Order>>()
            .await
            .unwrap();
        assert!(!db_orders[0].order_meta_data.invalidated);

        let cancellation_time =
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(1234567890, 0), Utc);
        db.cancel_order(&order.order_meta_data.uid, cancellation_time)
            .await
            .unwrap();
        let db_orders = db
            .orders(&filter)
            .try_collect::<Vec<Order>>()
            .await
            .unwrap();
        assert!(db_orders[0].order_meta_data.invalidated);

        let query = "SELECT cancellation_timestamp FROM orders;";
        let first_cancellation: CancellationQueryRow =
            sqlx::query_as(query).fetch_one(&db.pool).await.unwrap();
        assert_eq!(cancellation_time, first_cancellation.cancellation_timestamp);

        // Cancel again and verify that cancellation timestamp was not changed.
        let irrelevant_time = DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp(1234567890, 1_000_000_000),
            Utc,
        );

        assert_ne!(
            irrelevant_time, cancellation_time,
            "Expected cancellation times to be different."
        );

        db.cancel_order(&order.order_meta_data.uid, irrelevant_time)
            .await
            .unwrap();
        let second_cancellation: CancellationQueryRow =
            sqlx::query_as(query).fetch_one(&db.pool).await.unwrap();
        assert_eq!(first_cancellation, second_cancellation);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_filter_orders_by_address() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let orders = vec![
            Order {
                order_meta_data: OrderMetaData {
                    owner: H160::from_low_u64_be(0),
                    uid: OrderUid([0u8; 56]),
                    status: OrderStatus::Expired,
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
                    status: OrderStatus::Expired,
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
                    status: OrderStatus::Expired,
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

        async fn assert_orders(db: &Postgres, filter: &OrderFilter, expected: &[Order]) {
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
                owner: Some(owner),
                ..Default::default()
            },
            &orders[0..2],
        )
        .await;

        let sell_token = H160::from_low_u64_be(1);
        assert_orders(
            &db,
            &OrderFilter {
                sell_token: Some(sell_token),
                ..Default::default()
            },
            &orders[0..3],
        )
        .await;

        let buy_token = H160::from_low_u64_be(3);
        assert_orders(
            &db,
            &OrderFilter {
                buy_token: Some(buy_token),
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

        assert_orders(
            &db,
            &OrderFilter {
                uid: Some(orders[0].order_meta_data.uid),
                ..Default::default()
            },
            &orders[0..1],
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_filter_orders_by_fully_executed() {
        let db = Postgres::new("postgresql://").unwrap();
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

        db.append_events_(vec![(
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Event::Trade(Trade {
                order_uid: order.order_meta_data.uid,
                sell_amount_including_fee: 3.into(),
                ..Default::default()
            }),
        )])
        .await
        .unwrap();
        let order = get_order(true).await.unwrap().unwrap();
        assert_eq!(
            order.order_meta_data.executed_sell_amount,
            BigUint::from(3u8)
        );

        db.append_events_(vec![(
            EventIndex {
                block_number: 1,
                log_index: 0,
            },
            Event::Trade(Trade {
                order_uid: order.order_meta_data.uid,
                sell_amount_including_fee: 6.into(),
                ..Default::default()
            }),
        )])
        .await
        .unwrap();
        let order = get_order(true).await.unwrap().unwrap();
        assert_eq!(
            order.order_meta_data.executed_sell_amount,
            BigUint::from(9u8),
        );

        // The order disappears because it is fully executed.
        db.append_events_(vec![(
            EventIndex {
                block_number: 2,
                log_index: 0,
            },
            Event::Trade(Trade {
                order_uid: order.order_meta_data.uid,
                sell_amount_including_fee: 1.into(),
                ..Default::default()
            }),
        )])
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
        let db = Postgres::new("postgresql://").unwrap();
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
            db.append_events_(vec![(
                EventIndex {
                    block_number: i,
                    log_index: 0,
                },
                Event::Trade(Trade {
                    order_uid: order.order_meta_data.uid,
                    sell_amount_including_fee: U256::MAX,
                    ..Default::default()
                }),
            )])
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
        let db = Postgres::new("postgresql://").unwrap();
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

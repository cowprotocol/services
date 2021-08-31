use super::*;
use crate::conversions::*;
use anyhow::{anyhow, Context, Result};
use bigdecimal::{BigDecimal, Zero};
use chrono::{DateTime, Utc};
use const_format::concatcp;
use futures::stream::TryStreamExt;
use model::order::{BuyTokenDestination, SellTokenSource};
use model::{
    order::{Order, OrderCreation, OrderKind, OrderMetaData, OrderStatus, OrderUid},
    signature::{Signature, SigningScheme},
};
use primitive_types::H160;
use std::{borrow::Cow, convert::TryInto};

#[async_trait::async_trait]
pub trait OrderStoring: Send + Sync {
    async fn insert_order(&self, order: &Order) -> Result<(), InsertionError>;
    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()>;
    async fn orders(&self, filter: &OrderFilter) -> Result<Vec<Order>>;
    async fn single_order(&self, uid: &OrderUid) -> Result<Option<Order>>;
    async fn solvable_orders(&self, min_valid_to: u32) -> Result<Vec<Order>>;
    // Soon:
    // async fn user_orders(&self, filter, order_uid) -> Result<Vec<Order>>;
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

/// Source from which the sellAmount should be drawn upon order fulfilment
#[derive(sqlx::Type)]
#[sqlx(type_name = "SellTokenSource")]
#[sqlx(rename_all = "lowercase")]
pub enum DbSellTokenSource {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
    /// Internal balances to the Vault with GPv2 relayer approval
    External,
}

impl DbSellTokenSource {
    pub fn from(order_kind: SellTokenSource) -> Self {
        match order_kind {
            SellTokenSource::Erc20 => Self::Erc20,
            SellTokenSource::Internal => Self::Internal,
            SellTokenSource::External => Self::External,
        }
    }
    fn into(self) -> SellTokenSource {
        match self {
            Self::Erc20 => SellTokenSource::Erc20,
            Self::Internal => SellTokenSource::Internal,
            Self::External => SellTokenSource::External,
        }
    }
}

/// Destination for which the buyAmount should be transferred to order's receiver to upon fulfilment
#[derive(sqlx::Type)]
#[sqlx(type_name = "BuyTokenDestination")]
#[sqlx(rename_all = "lowercase")]
pub enum DbBuyTokenDestination {
    /// Pay trade proceeds as an ERC20 token transfer
    Erc20,
    /// Pay trade proceeds as a Vault internal balance transfer
    Internal,
}

impl DbBuyTokenDestination {
    pub fn from(order_kind: BuyTokenDestination) -> Self {
        match order_kind {
            BuyTokenDestination::Erc20 => Self::Erc20,
            BuyTokenDestination::Internal => Self::Internal,
        }
    }
    fn into(self) -> BuyTokenDestination {
        match self {
            Self::Erc20 => BuyTokenDestination::Erc20,
            Self::Internal => BuyTokenDestination::Internal,
        }
    }
}

#[derive(PartialEq, sqlx::Type)]
#[sqlx(type_name = "SigningScheme")]
#[sqlx(rename_all = "lowercase")]
pub enum DbSigningScheme {
    Eip712,
    EthSign,
    PreSign,
}

impl DbSigningScheme {
    pub fn from(signing_scheme: SigningScheme) -> Self {
        match signing_scheme {
            SigningScheme::Eip712 => Self::Eip712,
            SigningScheme::EthSign => Self::EthSign,
            SigningScheme::PreSign => Self::PreSign,
        }
    }

    fn into(self) -> SigningScheme {
        match self {
            Self::Eip712 => SigningScheme::Eip712,
            Self::EthSign => SigningScheme::EthSign,
            Self::PreSign => SigningScheme::PreSign,
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

// When querying orders we have several specialized use cases working with their own filtering,
// ordering, indexes. The parts that are shared between all queries are defined here so they can be
// reused.

const ORDERS_SELECT: &str = "\
    o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, o.buy_amount, \
    o.valid_to, o.app_data, o.fee_amount, o.kind, o.partially_fillable, o.signature, o.receiver, \
    o.signing_scheme, o.settlement_contract, o.sell_token_balance, o.buy_token_balance, \
    COALESCE(SUM(t.buy_amount), 0) AS sum_buy, \
    COALESCE(SUM(t.sell_amount), 0) AS sum_sell, \
    COALESCE(SUM(t.fee_amount), 0) AS sum_fee, \
    (COUNT(invalidations.*) > 0 OR o.cancellation_timestamp IS NOT NULL) AS invalidated, \
    (o.signing_scheme = 'presign' AND COALESCE(( \
        SELECT (NOT p.signed) as unsigned \
        FROM presignature_events p \
        WHERE o.uid = p.order_uid \
        ORDER BY p.block_number DESC, p.log_index DESC \
        LIMIT 1 \
    ), true)) AS presignature_pending \
";

const ORDERS_FROM: &str = "\
    orders o \
    LEFT OUTER JOIN trades t ON o.uid = t.order_uid \
    LEFT OUTER JOIN invalidations ON o.uid = invalidations.order_uid \
";

const ORDERS_GROUP_BY: &str = "o.uid ";

#[async_trait::async_trait]
impl OrderStoring for Postgres {
    async fn insert_order(&self, order: &Order) -> Result<(), InsertionError> {
        const QUERY: &str = "\
            INSERT INTO orders (
                uid, owner, creation_timestamp, sell_token, buy_token, receiver, sell_amount, buy_amount, \
                valid_to, app_data, fee_amount, kind, partially_fillable, signature, signing_scheme, \
                settlement_contract, sell_token_balance, buy_token_balance) \
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
            .bind(&*order.order_creation.signature.to_bytes())
            .bind(DbSigningScheme::from(
                order.order_creation.signature.scheme(),
            ))
            .bind(order.order_meta_data.settlement_contract.as_bytes())
            .bind(DbSellTokenSource::from(
                order.order_creation.sell_token_balance,
            ))
            .bind(DbBuyTokenDestination::from(
                order.order_creation.buy_token_balance,
            ))
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

    async fn orders(&self, filter: &OrderFilter) -> Result<Vec<Order>> {
        // The `or`s in the `where` clause are there so that each filter is ignored when not set.
        // We use a subquery instead of a `having` clause in the inner query because we would not be
        // able to use the `sum_*` columns there.
        #[rustfmt::skip]
        const QUERY: &str = concatcp!(
            "SELECT * FROM ( ",
                "SELECT ", ORDERS_SELECT,
                "FROM ", ORDERS_FROM,
                "WHERE \
                    o.valid_to >= $1 AND \
                    ($2 IS NULL OR o.owner = $2) AND \
                    ($3 IS NULL OR o.sell_token = $3) AND \
                    ($4 IS NULL OR o.buy_token = $4) AND \
                    ($5 IS NULL OR o.uid = $5) ",
                "GROUP BY ", ORDERS_GROUP_BY,
            ") AS unfiltered \
            WHERE \
                ($6 OR CASE kind \
                    WHEN 'sell' THEN sum_sell < sell_amount \
                    WHEN 'buy' THEN sum_buy < buy_amount \
                END) AND \
                ($7 OR NOT invalidated);"
        );
        sqlx::query_as(QUERY)
            .bind(filter.min_valid_to as i64)
            .bind(filter.owner.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.sell_token.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.buy_token.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.uid.as_ref().map(|uid| uid.0.as_ref()))
            .bind(!filter.exclude_fully_executed)
            .bind(!filter.exclude_invalidated)
            .fetch(&self.pool)
            .err_into()
            .and_then(|row: OrdersQueryRow| async move { row.into_order() })
            .try_collect()
            .await
    }

    async fn single_order(&self, uid: &OrderUid) -> Result<Option<Order>> {
        #[rustfmt::skip]
        const QUERY: &str = concatcp!(
            "SELECT ", ORDERS_SELECT,
            "FROM ", ORDERS_FROM,
            "WHERE o.uid = $1 ",
            "GROUP BY ", ORDERS_GROUP_BY,
        );
        let order = sqlx::query_as(QUERY)
            .bind(uid.0.as_ref())
            .fetch_optional(&self.pool)
            .await?;
        order.map(OrdersQueryRow::into_order).transpose()
    }

    async fn solvable_orders(&self, min_valid_to: u32) -> Result<Vec<Order>> {
        #[rustfmt::skip]
        const QUERY: &str = concatcp!(
            "SELECT * FROM ( ",
                "SELECT ", ORDERS_SELECT,
                "FROM ", ORDERS_FROM,
                "WHERE o.valid_to >= $1 ",
                "GROUP BY ", ORDERS_GROUP_BY,
            ") AS unfiltered \
            WHERE \
                CASE kind \
                    WHEN 'sell' THEN sum_sell < sell_amount \
                    WHEN 'buy' THEN sum_buy < buy_amount \
                END AND \
                (NOT invalidated);"
        );
        sqlx::query_as(QUERY)
            .bind(min_valid_to as i64)
            .fetch(&self.pool)
            .err_into()
            .and_then(|row: OrdersQueryRow| async move { row.into_order() })
            .try_collect()
            .await
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
    sell_token_balance: DbSellTokenSource,
    buy_token_balance: DbBuyTokenDestination,
    presignature_pending: bool,
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
        if self.presignature_pending {
            return OrderStatus::SignaturePending;
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
        let signing_scheme = self.signing_scheme.into();
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
            signature: Signature::from_bytes(signing_scheme, &self.signature)?,
            sell_token_balance: self.sell_token_balance.into(),
            buy_token_balance: self.buy_token_balance.into(),
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
    use num_bigint::BigUint;
    use primitive_types::U256;
    use shared::event_handling::EventIndex;
    use sqlx::Executor;
    use std::{
        collections::HashSet,
        sync::atomic::{AtomicI64, Ordering},
    };

    #[test]
    fn order_status() {
        let valid_to_timestamp = Utc::now() + Duration::days(1);

        let order_row = || OrdersQueryRow {
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
            sell_token_balance: DbSellTokenSource::External,
            buy_token_balance: DbBuyTokenDestination::Internal,
            presignature_pending: false,
        };

        // Open - sell (filled - 0%)
        assert_eq!(order_row().calculate_status(), OrderStatus::Open);

        // Open - sell (almost filled - 99.99%)
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(10_000),
                sum_sell: BigDecimal::from(9_999),
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Open
        );

        // Open - with presignature
        assert_eq!(
            OrdersQueryRow {
                signing_scheme: DbSigningScheme::PreSign,
                presignature_pending: false,
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Open
        );

        // SignaturePending - without presignature
        assert_eq!(
            OrdersQueryRow {
                signing_scheme: DbSigningScheme::PreSign,
                presignature_pending: true,
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::SignaturePending
        );

        // Filled - sell (filled - 100%)
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(3),
                sum_fee: BigDecimal::from(1),
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Fulfilled
        );

        // Open - buy (filled - 0%)
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(1),
                sum_buy: BigDecimal::from(0),
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Open
        );

        // Open - buy (almost filled - 99.99%)
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(10_000),
                sum_buy: BigDecimal::from(9_999),
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Open
        );

        // Filled - buy (filled - 100%)
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(1),
                sum_buy: BigDecimal::from(1),
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Fulfilled
        );

        // Cancelled - no fills - sell
        assert_eq!(
            OrdersQueryRow {
                invalidated: true,
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Cancelled
        );

        // Cancelled - partial fill - sell
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(1),
                sum_fee: BigDecimal::default(),
                invalidated: true,
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Cancelled
        );

        // Cancelled - partial fill - buy
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(2),
                sum_buy: BigDecimal::from(1),
                invalidated: true,
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Cancelled
        );

        // Expired - no fills
        let valid_to_yesterday = Utc::now() - Duration::days(1);

        assert_eq!(
            OrdersQueryRow {
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Expired
        );

        // Expired - partial fill - sell
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(1),
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Expired
        );

        // Expired - partial fill - buy
        assert_eq!(
            OrdersQueryRow {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(2),
                sum_buy: BigDecimal::from(1),
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Expired
        );

        // Expired - with pending presignature
        assert_eq!(
            OrdersQueryRow {
                signing_scheme: DbSigningScheme::PreSign,
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                presignature_pending: true,
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::Expired
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_same_order_twice_fails() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let mut order = Order::default();
        db.insert_order(&order).await.unwrap();

        // Note that order UIDs do not care about the signing scheme.
        order.order_creation.signature = Signature::default_with(SigningScheme::PreSign);
        assert!(matches!(
            db.insert_order(&order).await,
            Err(InsertionError::DuplicatedRecord)
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_order_roundtrip() {
        let db = Postgres::new("postgresql://").unwrap();
        for signing_scheme in &[
            SigningScheme::Eip712,
            SigningScheme::EthSign,
            SigningScheme::PreSign,
        ] {
            db.clear().await.unwrap();
            let filter = OrderFilter::default();
            assert!(db.orders(&filter).await.unwrap().is_empty());
            let order = Order {
                order_meta_data: OrderMetaData {
                    creation_date: DateTime::<Utc>::from_utc(
                        NaiveDateTime::from_timestamp(1234567890, 0),
                        Utc,
                    ),
                    status: match signing_scheme {
                        SigningScheme::PreSign => OrderStatus::SignaturePending,
                        _ => OrderStatus::Open,
                    },
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
                    signature: Signature::default_with(*signing_scheme),
                    sell_token_balance: SellTokenSource::Erc20,
                    buy_token_balance: BuyTokenDestination::Internal,
                },
            };
            db.insert_order(&order).await.unwrap();
            assert_eq!(db.orders(&filter).await.unwrap(), vec![order]);
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
        assert!(db.orders(&filter).await.unwrap().is_empty());

        let order = Order::default();
        db.insert_order(&order).await.unwrap();
        let db_orders = db.orders(&filter).await.unwrap();
        assert!(!db_orders[0].order_meta_data.invalidated);

        let cancellation_time =
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(1234567890, 0), Utc);
        db.cancel_order(&order.order_meta_data.uid, cancellation_time)
            .await
            .unwrap();
        let db_orders = db.orders(&filter).await.unwrap();
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
                .orders(filter)
                .await
                .unwrap()
                .into_iter()
                .collect::<HashSet<_>>();
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
                .await
                .unwrap()
                .into_iter()
                .next()
            }
        };

        let order = get_order(true).await.unwrap();
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
        let order = get_order(true).await.unwrap();
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
        let order = get_order(true).await.unwrap();
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
        let order = get_order(false).await.unwrap();
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
            .await
            .unwrap()
            .into_iter()
            .next()
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
            !db.orders(&OrderFilter {
                exclude_invalidated: true,
                ..Default::default()
            })
            .await
            .unwrap()
            .is_empty()
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

    #[tokio::test]
    #[ignore]
    async fn postgres_solvable_orders() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let order = Order {
            order_meta_data: Default::default(),
            order_creation: OrderCreation {
                kind: OrderKind::Sell,
                sell_amount: 10.into(),
                buy_amount: 100.into(),
                valid_to: 3,
                partially_fillable: true,
                ..Default::default()
            },
        };
        db.insert_order(&order).await.unwrap();

        let get_order = |min_valid_to| {
            let db = db.clone();
            async move {
                let orders = db.solvable_orders(min_valid_to).await.unwrap();
                orders.into_iter().next()
            }
        };

        // not solvable because valid to
        assert!(get_order(4).await.is_none());

        // not solvable because fully executed
        db.append_events_(vec![(
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Event::Trade(Trade {
                order_uid: order.order_meta_data.uid,
                sell_amount_including_fee: 10.into(),
                ..Default::default()
            }),
        )])
        .await
        .unwrap();
        assert!(get_order(0).await.is_none());
        db.replace_events_(0, Vec::new()).await.unwrap();

        // not solvable because invalidated
        db.append_events_(vec![(
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Event::Invalidation(Invalidation {
                order_uid: order.order_meta_data.uid,
            }),
        )])
        .await
        .unwrap();
        assert!(get_order(0).await.is_none());
        db.replace_events_(0, Vec::new()).await.unwrap();

        // solvable
        assert!(get_order(3).await.is_some());

        // still solvable because only partially filled
        db.append_events_(vec![(
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Event::Trade(Trade {
                order_uid: order.order_meta_data.uid,
                sell_amount_including_fee: 5.into(),
                ..Default::default()
            }),
        )])
        .await
        .unwrap();
        assert!(get_order(3).await.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_single_order() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let order0 = Order {
            order_meta_data: OrderMetaData {
                uid: OrderUid([1u8; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        let order1 = Order {
            order_meta_data: OrderMetaData {
                uid: OrderUid([2u8; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(order0.order_meta_data.uid != order1.order_meta_data.uid);
        db.insert_order(&order0).await.unwrap();
        db.insert_order(&order1).await.unwrap();

        let get_order = |uid| {
            let db = db.clone();
            async move { db.single_order(uid).await.unwrap() }
        };

        assert!(get_order(&order0.order_meta_data.uid).await.is_some());
        assert!(get_order(&order1.order_meta_data.uid).await.is_some());
        assert!(get_order(&OrderUid::default()).await.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_presignature_status() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let uid = OrderUid([0u8; 56]);
        let order = Order {
            order_creation: OrderCreation {
                signature: Signature::default_with(SigningScheme::PreSign),
                ..Default::default()
            },
            order_meta_data: OrderMetaData {
                uid,
                ..Default::default()
            },
        };
        db.insert_order(&order).await.unwrap();

        let order_status = || async {
            db.orders(&OrderFilter {
                uid: Some(uid),
                ..Default::default()
            })
            .await
            .unwrap()[0]
                .order_meta_data
                .status
        };
        let block_number = AtomicI64::new(0);
        let insert_presignature = |signed: bool| {
            let db = db.clone();
            let block_number = &block_number;
            let owner = order.order_meta_data.owner.as_bytes();
            async move {
                sqlx::query(
                    "INSERT INTO presignature_events \
                    (block_number, log_index, owner, order_uid, signed) \
                 VALUES \
                    ($1, $2, $3, $4, $5)",
                )
                .bind(block_number.fetch_add(1, Ordering::SeqCst))
                .bind(0i64)
                .bind(owner)
                .bind(&uid.0[..])
                .bind(signed)
                .execute(&db.pool)
                .await
                .unwrap();
            }
        };

        // "presign" order with no signature events has pending status.
        assert_eq!(order_status().await, OrderStatus::SignaturePending);

        // Inserting a presignature event changes the order status.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);

        // "unsigning" the presignature makes the signature pending again.
        insert_presignature(false).await;
        assert_eq!(order_status().await, OrderStatus::SignaturePending);

        // Multiple "unsign" events keep the signature pending.
        insert_presignature(false).await;
        assert_eq!(order_status().await, OrderStatus::SignaturePending);

        // Re-signing sets the status back to open.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);

        // Re-signing sets the status back to open.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);
    }
}

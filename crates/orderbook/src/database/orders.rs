use super::*;
use crate::{conversions::*, fee_subsidy::FeeParameters};
use anyhow::{anyhow, Context as _, Result};
use chrono::{DateTime, Utc};
use const_format::concatcp;
use ethcontract::H256;
use futures::{stream::TryStreamExt, FutureExt};
use model::{
    app_id::AppId,
    order::{
        BuyTokenDestination, Order, OrderData, OrderKind, OrderMetadata, OrderStatus, OrderUid,
        SellTokenSource,
    },
    signature::{Signature, SigningScheme},
};
use num::Zero;
use primitive_types::H160;
use sqlx::{types::BigDecimal, Connection};
use std::{borrow::Cow, convert::TryInto};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait OrderStoring: Send + Sync {
    async fn insert_order(&self, order: &Order, fee: FeeParameters) -> Result<(), InsertionError>;
    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()>;
    async fn replace_order(
        &self,
        old_order: &OrderUid,
        new_order: &Order,
        new_fee: FeeParameters,
    ) -> Result<(), InsertionError>;
    // Legacy generic orders route that we are phasing out.
    async fn orders(&self, filter: &OrderFilter) -> Result<Vec<Order>>;
    async fn orders_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>>;
    async fn single_order(&self, uid: &OrderUid) -> Result<Option<Order>>;
    /// Orders that are solvable: minimum valid to, not fully executed, not invalidated.
    async fn solvable_orders(&self, min_valid_to: u32) -> Result<SolvableOrders>;
    /// All orders of a single user ordered by creation date descending (newest orders first).
    async fn user_orders(
        &self,
        owner: &H160,
        offset: u64,
        limit: Option<u64>,
    ) -> Result<Vec<Order>>;
}

pub struct SolvableOrders {
    pub orders: Vec<Order>,
    pub latest_settlement_block: u64,
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
    pub exclude_presignature_pending: bool,
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
    Eip1271,
    PreSign,
}

impl DbSigningScheme {
    pub fn from(signing_scheme: SigningScheme) -> Self {
        match signing_scheme {
            SigningScheme::Eip712 => Self::Eip712,
            SigningScheme::EthSign => Self::EthSign,
            SigningScheme::Eip1271 => Self::Eip1271,
            SigningScheme::PreSign => Self::PreSign,
        }
    }

    fn into(self) -> SigningScheme {
        match self {
            Self::Eip712 => SigningScheme::Eip712,
            Self::EthSign => SigningScheme::EthSign,
            Self::Eip1271 => SigningScheme::Eip1271,
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
//
// It might feel more natural to use aggregate, joins and group by to calculate trade data and
// invalidations. The problem with that is that it makes the query inefficient when we wish to limit
// or order the selected orders because postgres is unable to understand the query well enough.
// For example if we want to select orders ordered by creation timestamp on which there is an index
// with offset and limit then the group by causes postgres to not use the index for the ordering. So
// it will select orders, aggregate them with trades grouped by uid, sort by timestamp and only then
// apply the limit. Instead we would like to apply the limit first and only then aggregate but I
// could not get this to happen without writing the query using sub queries. A similar situation is
// discussed in https://dba.stackexchange.com/questions/88988/postgres-error-column-must-appear-in-the-group-by-clause-or-be-used-in-an-aggre .
//
// To analyze queries take a look at https://www.postgresql.org/docs/13/using-explain.html . I also
// find it useful to
// SET enable_seqscan = false;
// SET enable_nestloop = false;
// to get a better idea of what indexes postgres *could* use even if it decides that with the
// current amount of data this wouldn't be better.
const ORDERS_SELECT: &str = "\
    o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, o.buy_amount, \
    o.valid_to, o.app_data, o.fee_amount, o.full_fee_amount, o.kind, o.partially_fillable, o.signature, \
    o.receiver, o.signing_scheme, o.settlement_contract, o.sell_token_balance, o.buy_token_balance, \
    o.is_liquidity_order, \
    (SELECT COALESCE(SUM(t.buy_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_buy, \
    (SELECT COALESCE(SUM(t.sell_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_sell, \
    (SELECT COALESCE(SUM(t.fee_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_fee, \
    (o.cancellation_timestamp IS NOT NULL OR \
        (SELECT COUNT(*) FROM invalidations WHERE invalidations.order_uid = o.uid) > 0 \
    ) AS invalidated, \
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
";

async fn insert_order(
    order: &Order,
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), InsertionError> {
    const QUERY: &str = "\
            INSERT INTO orders (
                uid, owner, creation_timestamp, sell_token, buy_token, receiver, sell_amount, buy_amount, \
                valid_to, app_data, fee_amount, kind, partially_fillable, signature, signing_scheme, \
                settlement_contract, sell_token_balance, buy_token_balance, full_fee_amount, is_liquidity_order) \
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20);";
    let receiver = order
        .data
        .receiver
        .map(|address| address.as_bytes().to_vec());
    sqlx::query(QUERY)
        .bind(order.metadata.uid.0.as_ref())
        .bind(order.metadata.owner.as_bytes())
        .bind(order.metadata.creation_date)
        .bind(order.data.sell_token.as_bytes())
        .bind(order.data.buy_token.as_bytes())
        .bind(receiver)
        .bind(u256_to_big_decimal(&order.data.sell_amount))
        .bind(u256_to_big_decimal(&order.data.buy_amount))
        .bind(order.data.valid_to as i64)
        .bind(&order.data.app_data.0[..])
        .bind(u256_to_big_decimal(&order.data.fee_amount))
        .bind(DbOrderKind::from(order.data.kind))
        .bind(order.data.partially_fillable)
        .bind(&*order.signature.to_bytes())
        .bind(DbSigningScheme::from(order.signature.scheme()))
        .bind(order.metadata.settlement_contract.as_bytes())
        .bind(DbSellTokenSource::from(order.data.sell_token_balance))
        .bind(DbBuyTokenDestination::from(order.data.buy_token_balance))
        .bind(u256_to_big_decimal(&order.metadata.full_fee_amount))
        .bind(order.metadata.is_liquidity_order)
        .execute(transaction)
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

async fn insert_fee(
    uid: &OrderUid,
    fee: &FeeParameters,
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), InsertionError> {
    const QUERY: &str = "\
                INSERT INTO order_fee_parameters (\
                    order_uid, gas_amount, gas_price, sell_token_price) \
                VALUES($1, $2, $3, $4)\
            ;";
    sqlx::query(QUERY)
        .bind(uid.0.as_ref())
        .bind(fee.gas_amount)
        .bind(fee.gas_price)
        .bind(fee.sell_token_price)
        .execute(transaction)
        .await
        .map(|_| ())
        .map_err(InsertionError::DbError)
}

async fn cancel_order(
    order_uid: &OrderUid,
    timestamp: DateTime<Utc>,
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), sqlx::Error> {
    // We do not overwrite previously cancelled orders,
    // but this query does allow the user to soft cancel
    // an order that has already been invalidated on-chain.
    const QUERY: &str = "\
            UPDATE orders
            SET cancellation_timestamp = $1 \
            WHERE uid = $2\
            AND cancellation_timestamp IS NULL;";
    sqlx::query(QUERY)
        .bind(timestamp)
        .bind(order_uid.0.as_ref())
        .execute(transaction)
        .await
        .map(|_| ())
}

#[async_trait::async_trait]
impl OrderStoring for Postgres {
    async fn insert_order(&self, order: &Order, fee: FeeParameters) -> Result<(), InsertionError> {
        let order = order.clone();
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    insert_order(&order, transaction).await?;
                    insert_fee(&order.metadata.uid, &fee, transaction).await?;
                    Ok(())
                }
                .boxed()
            })
            .await
    }

    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()> {
        let order_uid = *order_uid;
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move { cancel_order(&order_uid, now, transaction).await }.boxed()
            })
            .await
            .context("cancel_order failed")
    }

    async fn replace_order(
        &self,
        old_order: &model::order::OrderUid,
        new_order: &model::order::Order,
        new_fee: FeeParameters,
    ) -> anyhow::Result<(), super::orders::InsertionError> {
        let old_order = *old_order;
        let new_order = new_order.clone();
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    cancel_order(&old_order, new_order.metadata.creation_date, transaction).await?;
                    insert_order(&new_order, transaction).await?;
                    insert_fee(&new_order.metadata.uid, &new_fee, transaction).await?;
                    Ok(())
                }
                .boxed()
            })
            .await
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
            ") AS unfiltered \
            WHERE \
                ($6 OR CASE kind \
                    WHEN 'sell' THEN sum_sell < sell_amount \
                    WHEN 'buy' THEN sum_buy < buy_amount \
                END) AND \
                ($7 OR NOT invalidated) AND \
                ($8 OR NOT presignature_pending);"
        );
        sqlx::query_as(QUERY)
            .bind(filter.min_valid_to as i64)
            .bind(filter.owner.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.sell_token.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.buy_token.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.uid.as_ref().map(|uid| uid.0.as_ref()))
            .bind(!filter.exclude_fully_executed)
            .bind(!filter.exclude_invalidated)
            .bind(!filter.exclude_presignature_pending)
            .fetch(&self.pool)
            .err_into()
            .and_then(|row: OrdersQueryRow| async move { row.into_order() })
            .try_collect()
            .await
    }

    async fn orders_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>> {
        // TODO - This query assumes there is only one settlement per block.
        //  when there are two, we would want all trades for which the log index is between
        //  that of the correct settlement and the next. For this we would have to
        //  - fetch all settlements for the block containing the specified txHash
        //  - sort them by log index
        //  - pick out the target settlement and get all trades with log index between target's and next.
        //  I believe this would require a string of queries something like
        // with target_block_number as (
        //     SELECT block_number from settlements where tx_hash = $1
        // ),
        // with next_log_index as (
        //     SELECT log_index from settlements
        //     WHERE block_number > target_block_number
        //     ORDER BY block_number asc
        //     LIMIT 1
        // )
        // "SELECT ", ORDERS_SELECT,
        // "FROM ", ORDERS_FROM,
        // "JOIN trades t \
        //     ON t.order_uid = o.uid \
        //  JOIN settlements s \
        //     ON s.block_number = t.block_number \
        //  WHERE s.tx_hash = $1 \
        //  AND t.log_index BETWEEN s.log_index AND next_log_index"
        #[rustfmt::skip]
        const QUERY: &str = concatcp!(
            "SELECT ", ORDERS_SELECT,
            "FROM ", ORDERS_FROM,
            "JOIN trades t \
                ON t.order_uid = o.uid \
             JOIN settlements s \
                ON s.block_number = t.block_number \
             WHERE s.tx_hash = $1 ",
        );
        sqlx::query_as(QUERY)
            .bind(tx_hash.0.as_ref())
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
        );
        let order = sqlx::query_as(QUERY)
            .bind(uid.0.as_ref())
            .fetch_optional(&self.pool)
            .await?;
        order.map(OrdersQueryRow::into_order).transpose()
    }

    async fn solvable_orders(&self, min_valid_to: u32) -> Result<SolvableOrders> {
        #[rustfmt::skip]
        const QUERY: &str = concatcp!(
            "SELECT * FROM ( ",
                "SELECT ", ORDERS_SELECT,
                "FROM ", ORDERS_FROM,
                "WHERE o.valid_to >= $1 ",
            ") AS unfiltered \
            WHERE \
                CASE kind \
                    WHEN 'sell' THEN sum_sell < sell_amount \
                    WHEN 'buy' THEN sum_buy < buy_amount \
                END AND \
                (NOT invalidated) AND \
                (NOT presignature_pending);"
        );
        let mut connection = self.pool.acquire().await?;

        connection
            .transaction(move |transaction| {
                async move {
                    let orders = sqlx::query_as(QUERY)
                        .bind(min_valid_to as i64)
                        .fetch(&mut *transaction)
                        .err_into()
                        .and_then(|row: OrdersQueryRow| async move { row.into_order() })
                        .try_collect()
                        .await?;
                    let settlement: i64 = sqlx::query_scalar(
                        "SELECT COALESCE(MAX(block_number), 0) FROM settlements",
                    )
                    .fetch_one(&mut *transaction)
                    .await?;
                    Ok(SolvableOrders {
                        orders,
                        latest_settlement_block: settlement as u64,
                    })
                }
                .boxed()
            })
            .await
    }

    async fn user_orders(
        &self,
        owner: &H160,
        offset: u64,
        limit: Option<u64>,
    ) -> Result<Vec<Order>> {
        // As a future consideration for this query we could move from offset to an approach called
        // keyset pagination where the offset is identified by "key" of the previous query. In our
        // case that would be the lowest creation_timestamp. This way the database can start
        // immediately at the offset through the index without enumerating the first N elements
        // before as is the case with OFFSET.
        // On the other hand that approach is less flexible so we will consider if we see that these
        // queries are taking too long in practice.
        #[rustfmt::skip]
        const QUERY: &str = concatcp!(
            "SELECT ", ORDERS_SELECT,
            "FROM ", ORDERS_FROM,
            "WHERE o.owner = $1 ",
            "ORDER BY o.creation_timestamp DESC ",
            "LIMIT $2 ",
            "OFFSET $3 ",
        );
        sqlx::query_as(QUERY)
            .bind(owner.as_bytes())
            .bind(limit.map(|limit| limit as i64))
            .bind(offset as i64)
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
    full_fee_amount: BigDecimal,
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
    is_liquidity_order: bool,
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
            return OrderStatus::PresignaturePending;
        }
        OrderStatus::Open
    }

    fn into_order(self) -> Result<Order> {
        let status = self.calculate_status();
        let metadata = OrderMetadata {
            creation_date: self.creation_timestamp,
            owner: h160_from_vec(self.owner)?,
            uid: OrderUid(
                self.uid
                    .try_into()
                    .map_err(|_| anyhow!("order uid has wrong length"))?,
            ),
            available_balance: Default::default(),
            executed_buy_amount: big_decimal_to_big_uint(&self.sum_buy)
                .context("executed buy amount is not an unsigned integer")?,
            executed_sell_amount: big_decimal_to_big_uint(&self.sum_sell)
                .context("executed sell amount is not an unsigned integer")?,
            // Executed fee amounts and sell amounts before fees are capped by
            // order's fee and sell amounts, and thus can always fit in a `U256`
            // - as it is limited by the order format.
            executed_sell_amount_before_fees: big_decimal_to_u256(&(self.sum_sell - &self.sum_fee))
                .context("executed sell amount before fees does not fit in a u256")?,
            executed_fee_amount: big_decimal_to_u256(&self.sum_fee)
                .context("executed fee amount is not a valid u256")?,
            invalidated: self.invalidated,
            status,
            settlement_contract: h160_from_vec(self.settlement_contract)?,
            full_fee_amount: big_decimal_to_u256(&self.full_fee_amount)
                .ok_or_else(|| anyhow!("full_fee_amount is not U256"))?,
            is_liquidity_order: self.is_liquidity_order,
        };
        let signing_scheme = self.signing_scheme.into();
        let data = OrderData {
            sell_token: h160_from_vec(self.sell_token)?,
            buy_token: h160_from_vec(self.buy_token)?,
            receiver: self.receiver.map(h160_from_vec).transpose()?,
            sell_amount: big_decimal_to_u256(&self.sell_amount)
                .ok_or_else(|| anyhow!("sell_amount is not U256"))?,
            buy_amount: big_decimal_to_u256(&self.buy_amount)
                .ok_or_else(|| anyhow!("buy_amount is not U256"))?,
            valid_to: self.valid_to.try_into().context("valid_to is not u32")?,
            app_data: AppId(
                self.app_data
                    .try_into()
                    .map_err(|_| anyhow!("app_data is not [u8; 32]"))?,
            ),
            fee_amount: big_decimal_to_u256(&self.fee_amount)
                .ok_or_else(|| anyhow!("fee_amount is not U256"))?,
            kind: self.kind.into(),
            partially_fillable: self.partially_fillable,
            sell_token_balance: self.sell_token_balance.into(),
            buy_token_balance: self.buy_token_balance.into(),
        };
        let signature = Signature::from_bytes(signing_scheme, &self.signature)?;
        Ok(Order {
            metadata,
            data,
            signature,
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
    use num::BigUint;
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
            full_fee_amount: BigDecimal::default(),
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
            is_liquidity_order: true,
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

        // PresignaturePending - without presignature
        assert_eq!(
            OrdersQueryRow {
                signing_scheme: DbSigningScheme::PreSign,
                presignature_pending: true,
                ..order_row()
            }
            .calculate_status(),
            OrderStatus::PresignaturePending
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
        db.insert_order(&order, Default::default()).await.unwrap();

        // Note that order UIDs do not care about the signing scheme.
        order.signature = Signature::default_with(SigningScheme::PreSign);
        assert!(matches!(
            db.insert_order(&order, Default::default()).await,
            Err(InsertionError::DuplicatedRecord)
        ));
    }

    #[tokio::test]
    #[ignore]
    #[allow(clippy::float_cmp)]
    async fn postgres_insert_fee() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let order = Order::default();
        let fee = FeeParameters {
            gas_amount: 1.,
            gas_price: 2.,
            sell_token_price: 3.,
        };
        db.insert_order(&order, fee).await.unwrap();
        let query = "SELECT * FROM order_fee_parameters;";
        let (uid, gas_amount, gas_price, sell_token_price): (Vec<u8>, f64, f64, f64) =
            sqlx::query_as(query)
                .bind(order.metadata.uid.0.as_ref())
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(uid, order.metadata.uid.0.as_ref());
        assert_eq!(gas_amount, 1.);
        assert_eq!(gas_price, 2.);
        assert_eq!(sell_token_price, 3.);
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
                metadata: OrderMetadata {
                    creation_date: DateTime::<Utc>::from_utc(
                        NaiveDateTime::from_timestamp(1234567890, 0),
                        Utc,
                    ),
                    status: match signing_scheme {
                        SigningScheme::PreSign => OrderStatus::PresignaturePending,
                        _ => OrderStatus::Open,
                    },
                    ..Default::default()
                },
                data: OrderData {
                    sell_token: H160::from_low_u64_be(1),
                    buy_token: H160::from_low_u64_be(2),
                    receiver: Some(H160::from_low_u64_be(6)),
                    sell_amount: 3.into(),
                    buy_amount: U256::MAX,
                    valid_to: u32::MAX,
                    app_data: AppId([4; 32]),
                    fee_amount: 5.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: true,
                    sell_token_balance: SellTokenSource::Erc20,
                    buy_token_balance: BuyTokenDestination::Internal,
                },
                signature: Signature::default_with(*signing_scheme),
            };
            db.insert_order(&order, Default::default()).await.unwrap();
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
        db.insert_order(&order, Default::default()).await.unwrap();
        let db_orders = db.orders(&filter).await.unwrap();
        assert!(!db_orders[0].metadata.invalidated);

        let cancellation_time =
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(1234567890, 0), Utc);
        db.cancel_order(&order.metadata.uid, cancellation_time)
            .await
            .unwrap();
        let db_orders = db.orders(&filter).await.unwrap();
        assert!(db_orders[0].metadata.invalidated);

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

        db.cancel_order(&order.metadata.uid, irrelevant_time)
            .await
            .unwrap();
        let second_cancellation: CancellationQueryRow =
            sqlx::query_as(query).fetch_one(&db.pool).await.unwrap();
        assert_eq!(first_cancellation, second_cancellation);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_replace_order() {
        let owner = H160([0x77; 20]);

        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let old_order = Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([1; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&old_order, Default::default())
            .await
            .unwrap();

        let new_order = Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([2; 56]),
                creation_date: Utc::now(),
                ..Default::default()
            },
            ..Default::default()
        };
        db.replace_order(&old_order.metadata.uid, &new_order, Default::default())
            .await
            .unwrap();

        let order_statuses = db
            .user_orders(&owner, 0, None)
            .await
            .unwrap()
            .iter()
            .map(|order| (order.metadata.uid, order.metadata.status))
            .collect::<Vec<_>>();
        assert_eq!(
            order_statuses,
            vec![
                (new_order.metadata.uid, OrderStatus::Open),
                (old_order.metadata.uid, OrderStatus::Cancelled),
            ]
        );

        let (old_order_cancellation,): (Option<DateTime<Utc>>,) =
            sqlx::query_as("SELECT cancellation_timestamp FROM orders;")
                .bind(old_order.metadata.uid.0.as_ref())
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(
            old_order_cancellation.unwrap().timestamp_millis(),
            new_order.metadata.creation_date.timestamp_millis(),
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_replace_order_no_cancellation_on_error() {
        let owner = H160([0x77; 20]);

        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let old_order = Order {
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([1; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&old_order, Default::default())
            .await
            .unwrap();

        let new_order = Order {
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([2; 56]),
                creation_date: Utc::now(),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&new_order, Default::default())
            .await
            .unwrap();

        // Attempt to replace an old order with one that already exists should fail.
        let err = db
            .replace_order(&old_order.metadata.uid, &new_order, Default::default())
            .await
            .unwrap_err();
        assert!(matches!(err, InsertionError::DuplicatedRecord));

        // Old order cancellation status should remain unchanged.
        let (old_order_cancellation,): (Option<DateTime<Utc>>,) =
            sqlx::query_as("SELECT cancellation_timestamp FROM orders;")
                .bind(old_order.metadata.uid.0.as_ref())
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(old_order_cancellation, None);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_filter_orders_by_address() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let orders = vec![
            Order {
                metadata: OrderMetadata {
                    owner: H160::from_low_u64_be(0),
                    uid: OrderUid([0u8; 56]),
                    status: OrderStatus::Expired,
                    ..Default::default()
                },
                data: OrderData {
                    sell_token: H160::from_low_u64_be(1),
                    buy_token: H160::from_low_u64_be(2),
                    valid_to: 10,
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                metadata: OrderMetadata {
                    owner: H160::from_low_u64_be(0),
                    uid: OrderUid([1; 56]),
                    status: OrderStatus::Expired,
                    ..Default::default()
                },
                data: OrderData {
                    sell_token: H160::from_low_u64_be(1),
                    buy_token: H160::from_low_u64_be(3),
                    valid_to: 11,
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                metadata: OrderMetadata {
                    owner: H160::from_low_u64_be(2),
                    uid: OrderUid([2u8; 56]),
                    status: OrderStatus::Expired,
                    ..Default::default()
                },
                data: OrderData {
                    sell_token: H160::from_low_u64_be(1),
                    buy_token: H160::from_low_u64_be(3),
                    valid_to: 12,
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        for order in orders.iter() {
            db.insert_order(order, Default::default()).await.unwrap();
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
                uid: Some(orders[0].metadata.uid),
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
            data: OrderData {
                kind: OrderKind::Sell,
                sell_amount: 10.into(),
                buy_amount: 100.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&order, Default::default()).await.unwrap();

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
        assert_eq!(order.metadata.executed_sell_amount, BigUint::from(0u8));

        db.append_events_(vec![(
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Event::Trade(Trade {
                order_uid: order.metadata.uid,
                sell_amount_including_fee: 3.into(),
                ..Default::default()
            }),
        )])
        .await
        .unwrap();
        let order = get_order(true).await.unwrap();
        assert_eq!(order.metadata.executed_sell_amount, BigUint::from(3u8));

        db.append_events_(vec![(
            EventIndex {
                block_number: 1,
                log_index: 0,
            },
            Event::Trade(Trade {
                order_uid: order.metadata.uid,
                sell_amount_including_fee: 6.into(),
                ..Default::default()
            }),
        )])
        .await
        .unwrap();
        let order = get_order(true).await.unwrap();
        assert_eq!(order.metadata.executed_sell_amount, BigUint::from(9u8),);

        // The order disappears because it is fully executed.
        db.append_events_(vec![(
            EventIndex {
                block_number: 2,
                log_index: 0,
            },
            Event::Trade(Trade {
                order_uid: order.metadata.uid,
                sell_amount_including_fee: 1.into(),
                ..Default::default()
            }),
        )])
        .await
        .unwrap();
        assert!(get_order(true).await.is_none());

        // If we include fully executed orders it is there.
        let order = get_order(false).await.unwrap();
        assert_eq!(order.metadata.executed_sell_amount, BigUint::from(10u8));

        // Change order type and see that is returned as not fully executed again.
        let query = "UPDATE orders SET kind = 'buy';";
        db.pool.execute(query).await.unwrap();
        assert!(get_order(true).await.is_some());
    }

    // In the schema we set the type of executed amounts in individual events to a 78 decimal digit
    // number. Summing over multiple events could overflow this because the smart contract only
    // guarantees that the filled amount (which amount that is depends on order type) does not
    // overflow a U256. This test shows that postgres does not error if this happens because
    // inside the SUM the number can have more digits. In particular:
    // - `executed_buy_amount` may overflow after repeated buys (since there is no upper bound)
    // - `executed_sell_amount` (with fees) may overflow since the total fits into a `U512`.
    #[tokio::test]
    #[ignore]
    async fn postgres_summed_executed_amount_does_not_overflow() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let order = Order {
            data: OrderData {
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&order, Default::default()).await.unwrap();

        let sell_amount_before_fees = U256::MAX / 16;
        let fee_amount = U256::MAX / 16;
        for i in 0..16 {
            db.append_events_(vec![(
                EventIndex {
                    block_number: i,
                    log_index: 0,
                },
                Event::Trade(Trade {
                    order_uid: order.metadata.uid,
                    sell_amount_including_fee: sell_amount_before_fees + fee_amount,
                    buy_amount: U256::MAX,
                    fee_amount,
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

        let expected_sell_amount_including_fees =
            u256_to_big_uint(&(sell_amount_before_fees + fee_amount)) * BigUint::from(16_u8);
        let expected_sell_amount_before_fees = sell_amount_before_fees * 16;
        let expected_buy_amount = u256_to_big_uint(&U256::MAX) * BigUint::from(16_u8);
        let expected_fee_amount = fee_amount * 16;

        assert!(order.metadata.executed_sell_amount > u256_to_big_uint(&U256::MAX));
        assert_eq!(
            order.metadata.executed_sell_amount,
            expected_sell_amount_including_fees
        );
        assert_eq!(
            order.metadata.executed_sell_amount_before_fees,
            expected_sell_amount_before_fees
        );
        assert!(expected_buy_amount.to_string().len() > 78);
        assert_eq!(order.metadata.executed_buy_amount, expected_buy_amount);
        assert_eq!(order.metadata.executed_fee_amount, expected_fee_amount);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_filter_orders_by_invalidated() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let uid = OrderUid([0u8; 56]);
        let order = Order {
            metadata: OrderMetadata {
                uid,
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&order, Default::default()).await.unwrap();

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
    async fn postgres_solvable_presign_orders() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let order = Order {
            data: OrderData {
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
            signature: Signature::default_with(SigningScheme::PreSign),
            ..Default::default()
        };
        db.insert_order(&order, Default::default()).await.unwrap();

        let get_order = || {
            let db = db.clone();
            async move {
                let orders = db.solvable_orders(0).await.unwrap();
                orders.orders.into_iter().next()
            }
        };
        let pre_signature_event = |block_number: u64, signed: bool| {
            let db = db.clone();
            let events = vec![(
                EventIndex {
                    block_number,
                    log_index: 0,
                },
                Event::PreSignature(PreSignature {
                    owner: order.metadata.owner,
                    order_uid: order.metadata.uid,
                    signed,
                }),
            )];
            async move {
                db.append_events_(events).await.unwrap();
            }
        };

        // not solvable because there is no presignature event.
        assert!(get_order().await.is_none());

        // solvable because once presignature event is observed.
        pre_signature_event(0, true).await;
        assert!(get_order().await.is_some());

        // not solvable because "unsigned" presignature event.
        pre_signature_event(1, false).await;
        assert!(get_order().await.is_none());

        // solvable once again because of new presignature event.
        pre_signature_event(2, true).await;
        assert!(get_order().await.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_solvable_orders_settlement_block() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        assert_eq!(
            db.solvable_orders(0).await.unwrap().latest_settlement_block,
            0
        );
        db.append_events_(vec![(
            EventIndex {
                block_number: 1,
                log_index: 0,
            },
            Event::Settlement(Settlement::default()),
        )])
        .await
        .unwrap();
        assert_eq!(
            db.solvable_orders(0).await.unwrap().latest_settlement_block,
            1
        );
        db.append_events_(vec![(
            EventIndex {
                block_number: 5,
                log_index: 3,
            },
            Event::Settlement(Settlement::default()),
        )])
        .await
        .unwrap();
        assert_eq!(
            db.solvable_orders(0).await.unwrap().latest_settlement_block,
            5
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_solvable_orders() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let order = Order {
            data: OrderData {
                kind: OrderKind::Sell,
                sell_amount: 10.into(),
                buy_amount: 100.into(),
                valid_to: 3,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&order, Default::default()).await.unwrap();

        let get_order = |min_valid_to| {
            let db = db.clone();
            async move {
                let orders = db.solvable_orders(min_valid_to).await.unwrap();
                orders.orders.into_iter().next()
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
                order_uid: order.metadata.uid,
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
                order_uid: order.metadata.uid,
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
                order_uid: order.metadata.uid,
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
            metadata: OrderMetadata {
                uid: OrderUid([1u8; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        let order1 = Order {
            metadata: OrderMetadata {
                uid: OrderUid([2u8; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(order0.metadata.uid != order1.metadata.uid);
        db.insert_order(&order0, Default::default()).await.unwrap();
        db.insert_order(&order1, Default::default()).await.unwrap();

        let get_order = |uid| {
            let db = db.clone();
            async move { db.single_order(uid).await.unwrap() }
        };

        assert!(get_order(&order0.metadata.uid).await.is_some());
        assert!(get_order(&order1.metadata.uid).await.is_some());
        assert!(get_order(&OrderUid::default()).await.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_presignature_status() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let uid = OrderUid([0u8; 56]);
        let order = Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid,
                ..Default::default()
            },
            signature: Signature::default_with(SigningScheme::PreSign),
        };
        db.insert_order(&order, Default::default()).await.unwrap();

        let order_status = || async {
            db.orders(&OrderFilter {
                uid: Some(uid),
                ..Default::default()
            })
            .await
            .unwrap()[0]
                .metadata
                .status
        };
        let block_number = AtomicI64::new(0);
        let insert_presignature = |signed: bool| {
            let db = db.clone();
            let block_number = &block_number;
            let owner = order.metadata.owner.as_bytes();
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
        assert_eq!(order_status().await, OrderStatus::PresignaturePending);

        // Inserting a presignature event changes the order status.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);

        // "unsigning" the presignature makes the signature pending again.
        insert_presignature(false).await;
        assert_eq!(order_status().await, OrderStatus::PresignaturePending);

        // Multiple "unsign" events keep the signature pending.
        insert_presignature(false).await;
        assert_eq!(order_status().await, OrderStatus::PresignaturePending);

        // Re-signing sets the status back to open.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);

        // Re-signing sets the status back to open.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_filter_presignature_pending() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        let order_uid = |uid: u8| OrderUid([uid; 56]);
        let order = |uid: u8, scheme: SigningScheme| Order {
            data: OrderData {
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid: order_uid(uid),
                ..Default::default()
            },
            signature: Signature::default_with(scheme),
        };

        db.insert_order(&order(0, SigningScheme::Eip712), Default::default())
            .await
            .unwrap();
        for i in 1..=4 {
            db.insert_order(&order(i, SigningScheme::PreSign), Default::default())
                .await
                .unwrap();
        }

        // Insert:
        // - No presignature events for order 0 (but its a ECDSA signed order)
        // - A signed event for order 1.
        // - No presignature events for order 2
        // - A signed and unsigned event for order 3
        // - A signed, unsigned and a final signed event for order 4
        sqlx::query(
            "INSERT INTO presignature_events \
                 (block_number, log_index, owner, order_uid, signed) \
             VALUES \
                 (0, 0, $1, $2, true), \
                 \
                 (1, 0, $1, $3, true), \
                 (2, 0, $1, $3, false),
                 \
                 (3, 0, $1, $4, true), \
                 (4, 0, $1, $4, false), \
                 (5, 0, $1, $4, true);",
        )
        .bind(&[0u8; 20][..])
        .bind(&order_uid(1).0[..])
        .bind(&order_uid(3).0[..])
        .bind(&order_uid(4).0[..])
        .execute(&db.pool)
        .await
        .unwrap();

        let query_order_uids = |filter: OrderFilter| {
            let db = db.clone();
            async move {
                let mut order_uids = db
                    .orders(&filter)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|order| order.metadata.uid)
                    .collect::<Vec<_>>();
                // Make sure the list is sorted, this makes assertions easier.
                order_uids.sort_by_key(|uid| uid.0);
                order_uids
            }
        };

        // With presignature pending filter, only orders that aren't waiting for
        // a presignature event are returned.
        let filtered_orders = query_order_uids(OrderFilter {
            exclude_presignature_pending: true,
            ..Default::default()
        })
        .await;
        assert_eq!(
            filtered_orders,
            [
                order_uid(0), // No presignature event, but its an ECDSA signed order
                order_uid(1), // Pre-sign order with pre-sign event
                order_uid(4), // Pre-sign order where the last event was a "signed" presignature
            ]
        );

        // Without presignature pending filter, all orders are returned.
        let unfiltered_orders = query_order_uids(OrderFilter {
            exclude_presignature_pending: false,
            ..Default::default()
        })
        .await;
        assert_eq!(
            unfiltered_orders,
            (0..=4).map(order_uid).collect::<Vec<_>>(),
        );
    }

    fn datetime(offset: u32) -> DateTime<Utc> {
        DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(offset as i64, 0), Utc)
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_user_orders() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let owners: Vec<H160> = (0u64..2).map(H160::from_low_u64_le).collect();

        let data = OrderData {
            valid_to: u32::MAX,
            ..Default::default()
        };
        let orders = [
            Order {
                data,
                metadata: OrderMetadata {
                    uid: OrderUid::from_integer(3),
                    owner: owners[0],
                    creation_date: datetime(3),
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                data,
                metadata: OrderMetadata {
                    uid: OrderUid::from_integer(1),
                    owner: owners[1],
                    creation_date: datetime(2),
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                data,
                metadata: OrderMetadata {
                    uid: OrderUid::from_integer(0),
                    owner: owners[0],
                    creation_date: datetime(1),
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                data,
                metadata: OrderMetadata {
                    uid: OrderUid::from_integer(2),
                    owner: owners[1],
                    creation_date: datetime(0),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        for order in &orders {
            db.insert_order(order, Default::default()).await.unwrap();
        }

        let result = db.user_orders(&owners[0], 0, None).await.unwrap();
        assert_eq!(result, vec![orders[0].clone(), orders[2].clone()]);

        let result = db.user_orders(&owners[1], 0, None).await.unwrap();
        assert_eq!(result, vec![orders[1].clone(), orders[3].clone()]);

        let result = db.user_orders(&owners[0], 0, Some(1)).await.unwrap();
        assert_eq!(result, vec![orders[0].clone()]);

        let result = db.user_orders(&owners[0], 1, Some(1)).await.unwrap();
        assert_eq!(result, vec![orders[2].clone()]);

        let result = db.user_orders(&owners[0], 2, Some(1)).await.unwrap();
        assert_eq!(result, vec![]);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_returns_expected_orders_for_tx_hash_request() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let orders: Vec<Order> = (0..=3)
            .map(|i| Order {
                data: OrderData {
                    valid_to: u32::MAX,
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    uid: OrderUid::from_integer(i),
                    ..Default::default()
                },
                ..Default::default()
            })
            .collect();

        // Each order was traded in the consecutive blocks.
        for (i, order) in orders.clone().iter().enumerate() {
            db.insert_order(order, Default::default()).await.unwrap();
            db.append_events_(vec![
                // Add settlement
                (
                    EventIndex {
                        block_number: i as u64,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: H256::from_low_u64_be(i as u64),
                    }),
                ),
                // Add trade
                (
                    EventIndex {
                        block_number: i as u64,
                        log_index: 1,
                    },
                    Event::Trade(Trade {
                        order_uid: order.metadata.uid,
                        ..Default::default()
                    }),
                ),
            ])
            .await
            .unwrap();
        }
        for (i, order) in orders.into_iter().enumerate() {
            let res = db
                .orders_for_tx(&H256::from_low_u64_be(i as u64))
                .await
                .unwrap();
            assert_eq!(res, vec![order]);
        }
    }
}

use {
    crate::{
        onchain_broadcasted_orders::OnchainOrderPlacementError,
        Address,
        AppId,
        OrderUid,
        TransactionHash,
    },
    futures::stream::BoxStream,
    sqlx::{
        types::{
            chrono::{DateTime, Utc},
            BigDecimal,
        },
        FromRow,
        PgConnection,
    },
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "OrderKind")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderKind {
    #[default]
    Buy,
    Sell,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "OrderClass")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderClass {
    #[default]
    Market,
    Liquidity,
    Limit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "SigningScheme")]
#[sqlx(rename_all = "lowercase")]
pub enum SigningScheme {
    #[default]
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

/// Source from which the sellAmount should be drawn upon order fulfilment
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "SellTokenSource")]
#[sqlx(rename_all = "lowercase")]
pub enum SellTokenSource {
    /// Direct ERC20 allowances to the Vault relayer contract
    #[default]
    Erc20,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
    /// Internal balances to the Vault with GPv2 relayer approval
    External,
}

/// Destination for which the buyAmount should be transferred to order's
/// receiver to upon fulfilment
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "BuyTokenDestination")]
#[sqlx(rename_all = "lowercase")]
pub enum BuyTokenDestination {
    /// Pay trade proceeds as an ERC20 token transfer
    #[default]
    Erc20,
    /// Pay trade proceeds as a Vault internal balance transfer
    Internal,
}

/// one row in the pre_interaction table
#[derive(Clone, Debug, Default, Eq, PartialEq, sqlx::FromRow)]
pub struct Interaction {
    pub target: Address,
    pub value: BigDecimal,
    pub data: Vec<u8>,
}

/// One row in the `orders` table.
#[derive(Clone, Debug, Default, Eq, PartialEq, sqlx::FromRow)]
pub struct Order {
    pub uid: OrderUid,
    pub owner: Address,
    pub creation_timestamp: DateTime<Utc>,
    pub sell_token: Address,
    pub buy_token: Address,
    pub receiver: Option<Address>,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub valid_to: i64,
    pub app_data: AppId,
    pub fee_amount: BigDecimal,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub signature: Vec<u8>,
    pub signing_scheme: SigningScheme,
    pub settlement_contract: Address,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub full_fee_amount: BigDecimal,
    pub cancellation_timestamp: Option<DateTime<Utc>>,
    pub class: OrderClass,
    pub surplus_fee: Option<BigDecimal>,
    pub surplus_fee_timestamp: Option<DateTime<Utc>>,
}

pub async fn insert_or_overwrite_pre_interactions(
    ex: &mut PgConnection,
    uid_and_pre_interaction: &[(OrderUid, Interaction)],
) -> Result<(), sqlx::Error> {
    for (index, (order_uid, pre_interaction)) in uid_and_pre_interaction.iter().enumerate() {
        insert_or_overwrite_pre_interaction(ex, index as i64, pre_interaction, order_uid).await?;
    }
    Ok(())
}

pub async fn insert_or_overwrite_pre_interaction(
    ex: &mut PgConnection,
    index: i64,
    pre_interaction: &Interaction,
    order_uid: &OrderUid,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO interactions (
    order_uid,
    index,
    target,
    value,
    data
)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (order_uid, index) DO UPDATE
SET target = $3,
value = $4, data = $5
    "#;
    sqlx::query(QUERY)
        .bind(&order_uid)
        .bind(&index)
        .bind(&pre_interaction.target)
        .bind(&pre_interaction.value)
        .bind(&pre_interaction.data)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn read_order_pre_interactions(
    ex: &mut PgConnection,
    id: &OrderUid,
) -> Result<Vec<Interaction>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT * FROM interactions
WHERE order_uid = $1
ORDER BY index
    "#;
    sqlx::query_as(QUERY).bind(id).fetch_all(ex).await
}

pub async fn insert_orders_and_ignore_conflicts(
    ex: &mut PgConnection,
    orders: &[Order],
) -> Result<(), sqlx::Error> {
    for order in orders {
        insert_order_and_ignore_conflicts(ex, order).await?;
    }
    Ok(())
}

const INSERT_ORDER_QUERY: &str = r#"
INSERT INTO orders (
    uid,
    owner,
    creation_timestamp,
    sell_token,
    buy_token,
    receiver,
    sell_amount,
    buy_amount,
    valid_to,
    app_data,
    fee_amount,
    kind,
    partially_fillable,
    signature,
    signing_scheme,
    settlement_contract,
    sell_token_balance,
    buy_token_balance,
    full_fee_amount,
    cancellation_timestamp,
    class,
    surplus_fee,
    surplus_fee_timestamp
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)
    "#;

pub async fn insert_order_and_ignore_conflicts(
    ex: &mut PgConnection,
    order: &Order,
) -> Result<(), sqlx::Error> {
    // To be used only for the ethflow contract order placement, where reorgs force
    // us to update orders
    // Since each order has a unique UID even after a reorg onchain placed orders
    // have the same data. Hence, we can disregard any conflicts.
    const QUERY: &str = const_format::concatcp!(INSERT_ORDER_QUERY, "ON CONFLICT (uid) DO NOTHING");
    insert_order_execute_sqlx(QUERY, ex, order).await
}

async fn insert_order_execute_sqlx(
    query_str: &str,
    ex: &mut PgConnection,
    order: &Order,
) -> Result<(), sqlx::Error> {
    sqlx::query(query_str)
        .bind(&order.uid)
        .bind(&order.owner)
        .bind(order.creation_timestamp)
        .bind(&order.sell_token)
        .bind(&order.buy_token)
        .bind(&order.receiver)
        .bind(&order.sell_amount)
        .bind(&order.buy_amount)
        .bind(order.valid_to)
        .bind(&order.app_data)
        .bind(&order.fee_amount)
        .bind(&order.kind)
        .bind(order.partially_fillable)
        .bind(order.signature.as_slice())
        .bind(order.signing_scheme)
        .bind(&order.settlement_contract)
        .bind(order.sell_token_balance)
        .bind(order.buy_token_balance)
        .bind(&order.full_fee_amount)
        .bind(order.cancellation_timestamp)
        .bind(order.class)
        .bind(&order.surplus_fee)
        .bind(order.surplus_fee_timestamp)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn insert_order(ex: &mut PgConnection, order: &Order) -> Result<(), sqlx::Error> {
    insert_order_execute_sqlx(INSERT_ORDER_QUERY, ex, order).await
}

pub async fn read_order(
    ex: &mut PgConnection,
    id: &OrderUid,
) -> Result<Option<Order>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT * FROM ORDERS
WHERE uid = $1
    "#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

pub fn is_duplicate_record_error(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_err) = &err {
        if let Some(code) = db_err.code() {
            return code.as_ref() == "23505";
        }
    }
    false
}

/// One row in the `order_quotes` table.
#[derive(Clone, Default, Debug, PartialEq, sqlx::FromRow)]
pub struct Quote {
    pub order_uid: OrderUid,
    pub gas_amount: f64,
    pub gas_price: f64,
    pub sell_token_price: f64,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
}

pub async fn insert_quotes(ex: &mut PgConnection, quotes: &[Quote]) -> Result<(), sqlx::Error> {
    for quote in quotes {
        insert_quote_and_update_on_conflict(ex, quote).await?;
    }
    Ok(())
}

const INSERT_ORDER_QUOTES_QUERY: &str = r#"
INSERT INTO order_quotes (
    order_uid,
    gas_amount,
    gas_price,
    sell_token_price,
    sell_amount,
    buy_amount
)
VALUES ($1, $2, $3, $4, $5, $6)"#;

pub async fn insert_quote_and_update_on_conflict(
    ex: &mut PgConnection,
    quote: &Quote,
) -> Result<(), sqlx::Error> {
    /// For ethflow orders, due to reorgs, different orders
    /// might be inserted with the same uid. Hence, we need
    /// to update quote entries in the database on conflicts
    const QUERY: &str = const_format::concatcp!(
        INSERT_ORDER_QUOTES_QUERY,
        " ON CONFLICT (order_uid) DO UPDATE
SET gas_amount = $2, gas_price = $3,
sell_token_price = $4, sell_amount = $5,
buy_amount = $6
    "
    );
    sqlx::query(QUERY)
        .bind(&quote.order_uid)
        .bind(quote.gas_amount)
        .bind(quote.gas_price)
        .bind(quote.sell_token_price)
        .bind(&quote.sell_amount)
        .bind(&quote.buy_amount)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn insert_quote(ex: &mut PgConnection, quote: &Quote) -> Result<(), sqlx::Error> {
    sqlx::query(INSERT_ORDER_QUOTES_QUERY)
        .bind(&quote.order_uid)
        .bind(quote.gas_amount)
        .bind(quote.gas_price)
        .bind(quote.sell_token_price)
        .bind(&quote.sell_amount)
        .bind(&quote.buy_amount)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn read_quote(
    ex: &mut PgConnection,
    id: &OrderUid,
) -> Result<Option<Quote>, sqlx::Error> {
    let query = r#"
SELECT * FROM order_quotes
WHERE order_uid = $1
"#;
    sqlx::query_as(query).bind(id).fetch_optional(ex).await
}

pub async fn cancel_order(
    ex: &mut PgConnection,
    order_uid: &OrderUid,
    timestamp: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    // We do not overwrite previously cancelled orders,
    // but this query does allow the user to soft cancel
    // an order that has already been invalidated on-chain.
    const QUERY: &str = r#"
UPDATE orders
SET cancellation_timestamp = $1
WHERE uid = $2
AND cancellation_timestamp IS NULL
    "#;
    sqlx::query(QUERY)
        .bind(timestamp)
        .bind(order_uid.0.as_ref())
        .execute(ex)
        .await
        .map(|_| ())
}

/// Order with extra information from other tables. Has all the information
/// needed to construct a model::Order.
#[derive(Debug, sqlx::FromRow)]
pub struct FullOrder {
    pub uid: OrderUid,
    pub owner: Address,
    pub creation_timestamp: DateTime<Utc>,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub valid_to: i64,
    pub app_data: AppId,
    pub fee_amount: BigDecimal,
    pub full_fee_amount: BigDecimal,
    pub kind: OrderKind,
    pub class: OrderClass,
    pub partially_fillable: bool,
    pub signature: Vec<u8>,
    pub sum_sell: BigDecimal,
    pub sum_buy: BigDecimal,
    pub sum_fee: BigDecimal,
    pub invalidated: bool,
    pub receiver: Option<Address>,
    pub signing_scheme: SigningScheme,
    pub settlement_contract: Address,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub presignature_pending: bool,
    pub pre_interactions: Vec<(Address, BigDecimal, Vec<u8>)>,
    pub ethflow_data: Option<(Option<TransactionHash>, i64)>,
    pub onchain_user: Option<Address>,
    pub onchain_placement_error: Option<OnchainOrderPlacementError>,
    pub surplus_fee: Option<BigDecimal>,
    pub surplus_fee_timestamp: Option<DateTime<Utc>>,
    pub executed_surplus_fee: Option<BigDecimal>,
    pub executed_solver_fee: Option<BigDecimal>,
}

impl FullOrder {
    pub fn valid_to(&self) -> i64 {
        if let Some((_, valid_to)) = self.ethflow_data {
            // For ethflow orders, we always return the user valid_to,
            // as the Eip1271 valid to is u32::max
            return valid_to;
        }
        self.valid_to
    }
}

// When querying orders we have several specialized use cases working with their
// own filtering, ordering, indexes. The parts that are shared between all
// queries are defined here so they can be reused.
//
// It might feel more natural to use aggregate, joins and group by to calculate
// trade data and invalidations. The problem with that is that it makes the
// query inefficient when we wish to limit or order the selected orders because
// postgres is unable to understand the query well enough. For example if we
// want to select orders ordered by creation timestamp on which there is an
// index with offset and limit then the group by causes postgres to not use the
// index for the ordering. So it will select orders, aggregate them with trades
// grouped by uid, sort by timestamp and only then apply the limit. Instead we
// would like to apply the limit first and only then aggregate but I
// could not get this to happen without writing the query using sub queries. A
// similar situation is discussed in https://dba.stackexchange.com/questions/88988/postgres-error-column-must-appear-in-the-group-by-clause-or-be-used-in-an-aggre .
//
// To analyze queries take a look at https://www.postgresql.org/docs/13/using-explain.html . I also
// find it useful to
// SET enable_seqscan = false;
// SET enable_nestloop = false;
// to get a better idea of what indexes postgres *could* use even if it decides
// that with the current amount of data this wouldn't be better.
//
// The pre_interactions are read as arrays of their fields: target, value, data.
// This is done as sqlx does not support reading arrays of more complicated
// types than just one field. The pre_interaction's data of target, value and
// data are composed to an array of interactions later.
const ORDERS_SELECT: &str = r#"
o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, o.buy_amount,
o.valid_to, o.app_data, o.fee_amount, o.full_fee_amount, o.kind, o.partially_fillable, o.signature,
o.receiver, o.signing_scheme, o.settlement_contract, o.sell_token_balance, o.buy_token_balance,
o.class, o.surplus_fee, o.surplus_fee_timestamp,
(SELECT COALESCE(SUM(t.buy_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_buy,
(SELECT COALESCE(SUM(t.sell_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_sell,
(SELECT COALESCE(SUM(t.fee_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_fee,
(o.cancellation_timestamp IS NOT NULL OR
    (SELECT COUNT(*) FROM invalidations WHERE invalidations.order_uid = o.uid) > 0 OR
    (SELECT COUNT(*) FROM onchain_order_invalidations onchain_c where onchain_c.uid = o.uid limit 1) > 0
) AS invalidated,
(o.signing_scheme = 'presign' AND COALESCE((
    SELECT (NOT p.signed) as unsigned
    FROM presignature_events p
    WHERE o.uid = p.order_uid
    ORDER BY p.block_number DESC, p.log_index DESC
    LIMIT 1
), true)) AS presignature_pending,
array(Select (p.target, p.value, p.data) from interactions p where p.order_uid = o.uid order by p.index) as pre_interactions,
(SELECT (tx_hash, eth_o.valid_to) from ethflow_orders eth_o
    left join ethflow_refunds on ethflow_refunds.order_uid=eth_o.uid
    where eth_o.uid = o.uid limit 1) as ethflow_data,
(SELECT onchain_o.sender from onchain_placed_orders onchain_o where onchain_o.uid = o.uid limit 1) as onchain_user,
(SELECT onchain_o.placement_error from onchain_placed_orders onchain_o where onchain_o.uid = o.uid limit 1) as onchain_placement_error,
(SELECT surplus_fee FROM order_execution oe WHERE oe.order_uid = o.uid ORDER BY oe.auction_id DESC LIMIT 1) as executed_surplus_fee,
(SELECT solver_fee FROM order_execution oe WHERE oe.order_uid = o.uid ORDER BY oe.auction_id DESC LIMIT 1) as executed_solver_fee
"#;

const ORDERS_FROM: &str = "orders o";

pub async fn single_full_order(
    ex: &mut PgConnection,
    uid: &OrderUid,
) -> Result<Option<FullOrder>, sqlx::Error> {
    #[rustfmt::skip]
        const QUERY: &str = const_format::concatcp!(
"SELECT ", ORDERS_SELECT,
" FROM ", ORDERS_FROM,
" WHERE o.uid = $1 ",
        );
    sqlx::query_as(QUERY).bind(uid).fetch_optional(ex).await
}

// Partial query for getting the log indices of events of a single settlement.
//
// This will fail if we ever have multiple settlements in the same transaction
// because this WITH query will return multiple rows. If we ever want to do
// this, a tx hash is no longer enough to uniquely identify a settlement so the
// "orders for tx hash" route needs to change in some way like taking block
// number and log index directly.
const SETTLEMENT_LOG_INDICES: &str = r#"
WITH
    -- The log index in this query is the log index from the settlement event, which comes after the trade events.
    settlement AS (
        SELECT block_number, log_index
        FROM settlements
        WHERE tx_hash = $1
    ),
    -- The log index in this query is the log index of the settlement event from the previous (lower log index) settlement in the same transaction or 0 if there is no previous settlement.
    previous_settlement AS (
        SELECT COALESCE(MAX(log_index), 0) AS low
        FROM settlements
        WHERE
            block_number = (SELECT block_number FROM settlement) AND
            log_index < (SELECT log_index FROM settlement)
    )
"#;

pub fn full_orders_in_tx<'a>(
    ex: &'a mut PgConnection,
    tx_hash: &'a TransactionHash,
) -> BoxStream<'a, Result<FullOrder, sqlx::Error>> {
    const QUERY: &str = const_format::formatcp!(
        r#"
{SETTLEMENT_LOG_INDICES}
SELECT {ORDERS_SELECT}
FROM {ORDERS_FROM}
JOIN trades t ON t.order_uid = o.uid
WHERE
    t.block_number = (SELECT block_number FROM settlement) AND
    -- BETWEEN is inclusive
    t.log_index BETWEEN (SELECT * from previous_settlement) AND (SELECT log_index FROM settlement)
;"#
    );
    sqlx::query_as(QUERY).bind(tx_hash).fetch(ex)
}

#[derive(Debug, sqlx::FromRow)]
pub struct OrderExecution {
    /// The `solver_fee` that got executed for this specific fill.
    pub executed_solver_fee: Option<BigDecimal>,
    pub sell_token: Address,
    pub buy_token: Address,
    pub kind: OrderKind,
    /// The entire `sell_amount` of the order.
    pub sell_amount: BigDecimal,
    /// The entire `buy_amount` of the order.
    pub buy_amount: BigDecimal,
    /// The amount that got executed just with this trade.
    pub executed_amount: BigDecimal,
    pub signature: Vec<u8>,
    pub signing_scheme: SigningScheme,
    pub owner: Address,
}

pub fn order_executions_in_tx<'a>(
    ex: &'a mut PgConnection,
    tx_hash: &'a TransactionHash,
) -> BoxStream<'a, Result<OrderExecution, sqlx::Error>> {
    const QUERY: &str = const_format::formatcp!(
        r#"
{SETTLEMENT_LOG_INDICES}
SELECT
    solver_fee AS executed_solver_fee,
    sell_token,
    buy_token,
    o.sell_amount,
    o.buy_amount AS buy_amount,
    kind,
    CASE
        WHEN o.kind = 'sell' THEN t.sell_amount
        ELSE t.buy_amount END AS executed_amount,
    o.owner,
    signature,
    signing_scheme
FROM order_execution AS oe
JOIN orders o ON o.uid = oe.order_uid
JOIN trades t ON t.order_uid = oe.order_uid
WHERE
    t.block_number = (SELECT block_number FROM settlement) AND
    t.log_index BETWEEN (SELECT * from previous_settlement) AND (SELECT log_index FROM settlement)
    "#
    );
    sqlx::query_as(QUERY).bind(tx_hash).fetch(ex)
}

pub fn user_orders<'a>(
    ex: &'a mut PgConnection,
    owner: &'a Address,
    offset: i64,
    limit: Option<i64>,
) -> BoxStream<'a, Result<FullOrder, sqlx::Error>> {
    // As a future consideration for this query we could move from offset to an
    // approach called keyset pagination where the offset is identified by "key"
    // of the previous query. In our case that would be the lowest
    // creation_timestamp. This way the database can start immediately at the
    // offset through the index without enumerating the first N elements
    // before as is the case with OFFSET.
    // On the other hand that approach is less flexible so we will consider if we
    // see that these queries are taking too long in practice.
    #[rustfmt::skip]
    const QUERY: &str = const_format::concatcp!(
"(SELECT ", ORDERS_SELECT,
" FROM ", ORDERS_FROM,
" LEFT OUTER JOIN onchain_placed_orders onchain_o on onchain_o.uid = o.uid",
" WHERE o.owner = $1",
" ORDER BY creation_timestamp DESC LIMIT $2 + $3 ) ",
" UNION ",
" (SELECT ", ORDERS_SELECT,
" FROM ", ORDERS_FROM,
" LEFT OUTER JOIN onchain_placed_orders onchain_o on onchain_o.uid = o.uid",
" WHERE onchain_o.sender = $1 ",
" ORDER BY creation_timestamp DESC LIMIT $2 + $3 ) ",
" ORDER BY creation_timestamp DESC ",
" LIMIT $2 ",
" OFFSET $3 ",
    );
    sqlx::query_as(QUERY)
        .bind(owner)
        .bind(limit)
        .bind(offset)
        .fetch(ex)
}

/// The base solvable orders query used in specialized queries. Parametrized by valid_to.
///
/// Excludes orders for the following conditions:
/// - valid_to is in the past
/// - fully executed
/// - cancelled on chain
/// - cancelled through API
/// - pending pre-signature
/// - ethflow specific invalidation conditions
#[rustfmt::skip]
const OPEN_ORDERS: &str = const_format::concatcp!(
"SELECT * FROM ( ",
    "SELECT ", ORDERS_SELECT,
    " FROM ", ORDERS_FROM,
    " LEFT OUTER JOIN ethflow_orders eth_o on eth_o.uid = o.uid ",
    " WHERE o.valid_to >= $1",
    " AND CASE WHEN eth_o.valid_to IS NULL THEN true ELSE eth_o.valid_to >= $1 END",
r#") AS unfiltered
WHERE
    CASE kind
        WHEN 'sell' THEN sum_sell < sell_amount
        WHEN 'buy' THEN sum_buy < buy_amount
    END AND
    (NOT invalidated) AND
    (onchain_placement_error IS NULL) AND
    (NOT presignature_pending)
"#
);

/// Uses the conditions from OPEN_ORDERS and checks the fok limit orders have
/// surplus fee.
pub fn solvable_orders(
    ex: &mut PgConnection,
    min_valid_to: i64,
    min_surplus_fee_timestamp: DateTime<Utc>,
) -> BoxStream<'_, Result<FullOrder, sqlx::Error>> {
    const QUERY: &str = const_format::concatcp!(
        OPEN_ORDERS,
        " AND (class <> 'limit' OR partially_fillable OR (surplus_fee IS NOT NULL AND \
         surplus_fee_timestamp > $2))"
    );
    sqlx::query_as(QUERY)
        .bind(min_valid_to)
        .bind(min_surplus_fee_timestamp)
        .fetch(ex)
}

pub async fn latest_settlement_block(ex: &mut PgConnection) -> Result<i64, sqlx::Error> {
    const QUERY: &str = r#"
SELECT COALESCE(MAX(block_number), 0)
FROM settlements
    "#;
    sqlx::query_scalar(QUERY).fetch_one(ex).await
}

/// Counts the number of limit orders with the conditions of OPEN_ORDERS. Used
/// to enforce a maximum number of limit orders per user.
pub async fn count_limit_orders_by_owner(
    ex: &mut PgConnection,
    min_valid_to: i64,
    owner: &Address,
) -> Result<i64, sqlx::Error> {
    const QUERY: &str = const_format::concatcp!(
        "SELECT COUNT (*) FROM (",
        OPEN_ORDERS,
        " AND class = 'limit'",
        " AND owner = $2",
        " ) AS subquery"
    );
    sqlx::query_scalar(QUERY)
        .bind(min_valid_to)
        .bind(owner)
        .fetch_one(ex)
        .await
}

/// These parameters have to match in an order for a [`FeeUpdate`] update to
/// apply to it.
#[derive(Clone, Debug, FromRow, PartialEq, Eq, Hash)]
pub struct OrderFeeSpecifier {
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: BigDecimal,
}

pub struct FeeUpdate {
    pub surplus_fee: Option<BigDecimal>,
    pub surplus_fee_timestamp: DateTime<Utc>,
    pub full_fee_amount: BigDecimal,
}

/// Updates the `surplus_fee` of multiple orders and returns their `uid`s.
pub async fn update_fok_limit_order_fees(
    ex: &mut PgConnection,
    order_spec: &OrderFeeSpecifier,
    update: &FeeUpdate,
) -> Result<Vec<OrderUid>, sqlx::Error> {
    const QUERY: &str = "
        UPDATE orders
        SET
            surplus_fee = $1,
            surplus_fee_timestamp = $2,
            full_fee_amount = $3
        WHERE
            sell_token = $4
            AND buy_token = $5
            AND sell_amount = $6
            AND NOT partially_fillable
        RETURNING
            uid
    ";
    sqlx::query_scalar(QUERY)
        .bind(&update.surplus_fee)
        .bind(update.surplus_fee_timestamp)
        .bind(&update.full_fee_amount)
        .bind(&order_spec.sell_token)
        .bind(&order_spec.buy_token)
        .bind(&order_spec.sell_amount)
        .fetch_all(ex)
        .await
}

/// All data required to filter, select and update orders to update the
/// `surplus_fee` for.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Default)]
pub struct OrderQuotingData {
    pub uid: OrderUid,
    pub owner: Address,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: BigDecimal,
    pub sell_token_balance: SellTokenSource,
    pub pre_interactions: i32,
}

/// Returns all fill or kill limit orders that are currently waiting to be
/// filled sorted by `surplus_fee_timestamp` with the most outdated ones coming
/// first.
pub fn open_fok_limit_orders(
    ex: &mut PgConnection,
    max_fee_timestamp: DateTime<Utc>,
    min_valid_to: i64,
) -> BoxStream<'_, Result<OrderQuotingData, sqlx::Error>> {
    const QUERY: &str = const_format::concatcp!(
        " SELECT sell_token, buy_token, sell_amount, uid, owner, sell_token_balance, \
         cardinality(pre_interactions) as pre_interactions",
        " FROM (",
        OPEN_ORDERS,
        "     AND class = 'limit'",
        "     AND NOT partially_fillable",
        "     AND COALESCE(surplus_fee_timestamp, 'epoch') < $2",
        "     ORDER BY surplus_fee_timestamp ASC NULLS FIRST",
        " ) as subquery"
    );

    sqlx::query_as(QUERY)
        .bind(min_valid_to)
        .bind(max_fee_timestamp)
        .fetch(ex)
}

/// Count the number of open limit orders. Used for metrics.
pub async fn count_fok_limit_orders(
    ex: &mut PgConnection,
    min_valid_to: i64,
) -> Result<i64, sqlx::Error> {
    const QUERY: &str = const_format::concatcp!(
        "SELECT COUNT (*) FROM (",
        OPEN_ORDERS,
        " AND class = 'limit'",
        " AND NOT partially_fillable",
        ") AS subquery"
    );
    sqlx::query_scalar(QUERY)
        .bind(min_valid_to)
        .fetch_one(ex)
        .await
}

/// Count the number of open fok limit orders with outdated fee as would be
/// returned by `open_fok_limit_orders`.
pub async fn count_fok_limit_orders_with_outdated_fees(
    ex: &mut PgConnection,
    max_fee_timestamp: DateTime<Utc>,
    min_valid_to: i64,
) -> Result<i64, sqlx::Error> {
    const QUERY: &str = const_format::concatcp!(
        "SELECT COUNT (*) FROM (",
        OPEN_ORDERS,
        " AND class = 'limit'",
        " AND NOT partially_fillable",
        " AND COALESCE(surplus_fee_timestamp, 'epoch') < $2",
        ") AS subquery"
    );
    sqlx::query_scalar(QUERY)
        .bind(min_valid_to)
        .bind(max_fee_timestamp)
        .fetch_one(ex)
        .await
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            byte_array::ByteArray,
            ethflow_orders::{
                insert_or_overwrite_ethflow_order,
                insert_refund_tx_hash,
                EthOrderPlacement,
                Refund,
            },
            events::{Event, EventIndex, Invalidation, PreSignature, Settlement, Trade},
            onchain_broadcasted_orders::{insert_onchain_order, OnchainOrderPlacement},
            onchain_invalidations::insert_onchain_invalidation,
            PgTransaction,
        },
        bigdecimal::num_bigint::{BigInt, ToBigInt},
        chrono::{TimeZone, Utc},
        futures::{StreamExt, TryStreamExt},
        sqlx::Connection,
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_order_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        insert_order(&mut db, &order).await.unwrap();
        let order_ = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert_eq!(order, order_);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_order_roundtrip_with_function_irgnoring_duplications() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        insert_order_and_ignore_conflicts(&mut db, &order)
            .await
            .unwrap();
        let order_ = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert_eq!(order, order_);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_onchain_user_order_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        let sender = ByteArray([3u8; 20]);
        insert_onchain_order(
            &mut db,
            &EventIndex::default(),
            &OnchainOrderPlacement {
                order_uid: OrderUid::default(),
                sender,
                placement_error: None,
            },
        )
        .await
        .unwrap();
        insert_order(&mut db, &order).await.unwrap();
        let order_ = single_full_order(&mut db, &order.uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(Some(sender), order_.onchain_user);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_ethflow_data_order_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        let user_valid_to = 4i64;
        insert_or_overwrite_ethflow_order(
            &mut db,
            &EthOrderPlacement {
                uid: OrderUid::default(),
                valid_to: user_valid_to,
            },
        )
        .await
        .unwrap();
        insert_order(&mut db, &order).await.unwrap();
        insert_refund_tx_hash(
            &mut db,
            &Refund {
                order_uid: order.uid,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let order_ = single_full_order(&mut db, &order.uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            Some((Some(Default::default()), user_valid_to)),
            order_.ethflow_data
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_order_roundtrip_pre_interactions() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        insert_order(&mut db, &order).await.unwrap();
        let pre_interaction_1 = Interaction::default();
        let pre_interaction_2 = Interaction {
            target: ByteArray([1; 20]),
            value: BigDecimal::new(10.into(), 1),
            data: vec![0u8, 1u8],
        };
        insert_or_overwrite_pre_interaction(&mut db, 0, &pre_interaction_1, &order.uid)
            .await
            .unwrap();
        insert_or_overwrite_pre_interaction(&mut db, 1, &pre_interaction_2, &order.uid)
            .await
            .unwrap();
        let order_ = single_full_order(&mut db, &order.uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            vec![ByteArray::default(), ByteArray([1; 20])],
            order_
                .pre_interactions
                .clone()
                .into_iter()
                .map(|v| v.0)
                .collect::<Vec<ByteArray<20>>>(),
        );
        assert_eq!(
            vec![BigDecimal::default(), BigDecimal::new(10.into(), 1)],
            order_
                .pre_interactions
                .clone()
                .into_iter()
                .map(|v| v.1)
                .collect::<Vec<BigDecimal>>()
        );
        assert_eq!(
            vec![vec![], vec![0u8, 1u8]],
            order_
                .pre_interactions
                .into_iter()
                .map(|v| v.2)
                .collect::<Vec<Vec<u8>>>()
        );
        let pre_interactions = read_order_pre_interactions(&mut db, &order.uid)
            .await
            .unwrap();
        assert_eq!(*pre_interactions.get(0).unwrap(), pre_interaction_1);
        assert_eq!(*pre_interactions.get(1).unwrap(), pre_interaction_2);

        let pre_interaction_overwrite = Interaction {
            target: ByteArray([2; 20]),
            value: BigDecimal::new(100.into(), 1),
            data: vec![0u8, 2u8],
        };
        insert_or_overwrite_pre_interaction(&mut db, 0, &pre_interaction_overwrite, &order.uid)
            .await
            .unwrap();
        let pre_interactions = read_order_pre_interactions(&mut db, &order.uid)
            .await
            .unwrap();
        assert_eq!(*pre_interactions.get(0).unwrap(), pre_interaction_overwrite);
        assert_eq!(*pre_interactions.get(1).unwrap(), pre_interaction_2);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_same_order_twice_fails() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        insert_order(&mut db, &order).await.unwrap();
        let err = insert_order(&mut db, &order).await.unwrap_err();
        assert!(is_duplicate_record_error(&err));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_same_order_twice_results_in_only_one_order() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        insert_order(&mut db, &order).await.unwrap();
        insert_order_and_ignore_conflicts(&mut db, &order)
            .await
            .unwrap();
        let order_ = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert_eq!(order, order_);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_orders_and_ignore_conflicts_ignores_the_conflict() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        insert_orders_and_ignore_conflicts(&mut db, vec![order.clone()].as_slice())
            .await
            .unwrap();
        insert_orders_and_ignore_conflicts(&mut db, vec![order].as_slice())
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_quote_roundtrip_updating_on_conflict() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let quote = Quote {
            order_uid: Default::default(),
            gas_amount: 1.,
            gas_price: 2.,
            sell_token_price: 3.,
            sell_amount: 4.into(),
            buy_amount: 5.into(),
        };
        insert_quote(&mut db, &quote).await.unwrap();
        insert_quote_and_update_on_conflict(&mut db, &quote)
            .await
            .unwrap();
        let quote_ = read_quote(&mut db, &quote.order_uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(quote, quote_);
        let mut quote2 = quote.clone();
        quote2.gas_amount = 2.0;
        insert_quote_and_update_on_conflict(&mut db, &quote2)
            .await
            .unwrap();
        let quote_ = read_quote(&mut db, &quote.order_uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(quote2, quote_);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_quote_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let quote = Quote {
            order_uid: Default::default(),
            gas_amount: 1.,
            gas_price: 2.,
            sell_token_price: 3.,
            sell_amount: 4.into(),
            buy_amount: 5.into(),
        };
        insert_quote(&mut db, &quote).await.unwrap();
        let quote_ = read_quote(&mut db, &quote.order_uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(quote, quote_);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_cancel_order() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order::default();
        insert_order(&mut db, &order).await.unwrap();
        let order = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert!(order.cancellation_timestamp.is_none());

        let time = Utc.timestamp_opt(1234567890, 0).unwrap();
        cancel_order(&mut db, &order.uid, time).await.unwrap();
        let order = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert_eq!(time, order.cancellation_timestamp.unwrap());

        // Cancel again and verify that cancellation timestamp was not changed.
        let irrelevant_time = Utc.timestamp_opt(1234567890, 1_000_000_000).unwrap();
        assert_ne!(irrelevant_time, time);
        cancel_order(&mut db, &order.uid, time).await.unwrap();
        let order = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert_eq!(time, order.cancellation_timestamp.unwrap());
    }

    // In the schema we set the type of executed amounts in individual events to a
    // 78 decimal digit number. Summing over multiple events could overflow this
    // because the smart contract only guarantees that the filled amount (which
    // amount that is depends on order type) does not overflow a U256. This test
    // shows that postgres does not error if this happens because inside the SUM
    // the number can have more digits. In particular:
    // - `executed_buy_amount` may overflow after repeated buys (since there is no
    //   upper bound)
    // - `executed_sell_amount` (with fees) may overflow since the total fits into a
    //   `U512`.
    #[tokio::test]
    #[ignore]
    async fn postgres_summed_executed_amount_does_not_overflow() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order {
            kind: OrderKind::Sell,
            ..Default::default()
        };
        insert_order(&mut db, &order).await.unwrap();

        let u256_max: BigInt = BigInt::from(2).pow(256) - 1;
        let sell_amount_before_fees: BigInt = u256_max.clone() / 16;
        let fee_amount: BigInt = u256_max.clone() / 16;
        let sell_amount_including_fee: BigInt =
            sell_amount_before_fees.clone() + fee_amount.clone();
        for i in 0..16 {
            crate::events::append(
                &mut db,
                &[(
                    EventIndex {
                        block_number: i,
                        log_index: 0,
                    },
                    Event::Trade(Trade {
                        order_uid: order.uid,
                        sell_amount_including_fee: sell_amount_including_fee.clone().into(),
                        buy_amount: u256_max.clone().into(),
                        fee_amount: fee_amount.clone().into(),
                    }),
                )],
            )
            .await
            .unwrap();
        }

        let order = single_full_order(&mut db, &order.uid)
            .await
            .unwrap()
            .unwrap();

        let expected_sell_amount_including_fees: BigInt = sell_amount_including_fee * 16;
        assert!(expected_sell_amount_including_fees > u256_max);
        let expected_sell_amount_before_fees: BigInt = sell_amount_before_fees * 16;
        let expected_buy_amount: BigInt = u256_max * 16;
        assert!(expected_buy_amount.to_string().len() > 78);
        let expected_fee_amount: BigInt = fee_amount * 16;

        assert_eq!(
            order.sum_sell.to_bigint().unwrap(),
            expected_sell_amount_including_fees
        );
        assert_eq!(
            (order.sum_sell - order.sum_fee.clone())
                .to_bigint()
                .unwrap(),
            expected_sell_amount_before_fees
        );
        assert_eq!(order.sum_buy.to_bigint().unwrap(), expected_buy_amount);
        assert_eq!(order.sum_fee.to_bigint().unwrap(), expected_fee_amount);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_solvable_presign_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order {
            sell_amount: 1.into(),
            buy_amount: 1.into(),
            signing_scheme: SigningScheme::PreSign,
            ..Default::default()
        };
        insert_order(&mut db, &order).await.unwrap();

        async fn get_order(ex: &mut PgConnection) -> Option<FullOrder> {
            solvable_orders(ex, 0, Utc::now())
                .next()
                .await
                .transpose()
                .unwrap()
        }

        async fn pre_signature_event(
            ex: &mut PgTransaction<'_>,
            block_number: i64,
            owner: Address,
            order_uid: OrderUid,
            signed: bool,
        ) {
            let events = [(
                EventIndex {
                    block_number,
                    log_index: 0,
                },
                Event::PreSignature(PreSignature {
                    owner,
                    order_uid,
                    signed,
                }),
            )];
            crate::events::append(ex, &events).await.unwrap()
        }

        // not solvable because there is no presignature event.
        assert!(get_order(&mut db).await.is_none());

        // solvable because once presignature event is observed.
        pre_signature_event(&mut db, 0, order.owner, order.uid, true).await;
        assert!(get_order(&mut db).await.is_some());

        // not solvable because "unsigned" presignature event.
        pre_signature_event(&mut db, 1, order.owner, order.uid, false).await;
        assert!(get_order(&mut db).await.is_none());

        // solvable once again because of new presignature event.
        pre_signature_event(&mut db, 2, order.owner, order.uid, true).await;
        assert!(get_order(&mut db).await.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_onchain_invalidated_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order {
            uid: ByteArray([2u8; 56]),
            kind: OrderKind::Sell,
            sell_amount: 10.into(),
            buy_amount: 100.into(),
            valid_to: 3,
            partially_fillable: true,
            ..Default::default()
        };
        insert_order(&mut db, &order).await.unwrap();
        let result = single_full_order(&mut db, &order.uid)
            .await
            .unwrap()
            .unwrap();
        assert!(!result.invalidated);
        insert_onchain_invalidation(&mut db, &EventIndex::default(), &order.uid)
            .await
            .unwrap();
        let result = single_full_order(&mut db, &order.uid)
            .await
            .unwrap()
            .unwrap();
        assert!(result.invalidated);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_solvable_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = Order {
            kind: OrderKind::Sell,
            sell_amount: 10.into(),
            buy_amount: 100.into(),
            valid_to: 3,
            partially_fillable: true,
            ..Default::default()
        };
        insert_order(&mut db, &order).await.unwrap();

        async fn get_order(ex: &mut PgConnection, min_valid_to: i64) -> Option<FullOrder> {
            solvable_orders(ex, min_valid_to, Utc::now())
                .next()
                .await
                .transpose()
                .unwrap()
        }

        // not solvable because valid to
        assert!(get_order(&mut db, 4).await.is_none());

        // not solvable because fully executed
        crate::events::append(
            &mut db,
            &[(
                EventIndex {
                    block_number: 0,
                    log_index: 0,
                },
                Event::Trade(Trade {
                    order_uid: order.uid,
                    sell_amount_including_fee: 10.into(),
                    ..Default::default()
                }),
            )],
        )
        .await
        .unwrap();
        assert!(get_order(&mut db, 0).await.is_none());
        crate::events::delete(&mut db, 0).await.unwrap();

        // not solvable because invalidated
        crate::events::append(
            &mut db,
            &[(
                EventIndex {
                    block_number: 0,
                    log_index: 0,
                },
                Event::Invalidation(Invalidation {
                    order_uid: order.uid,
                }),
            )],
        )
        .await
        .unwrap();
        assert!(get_order(&mut db, 0).await.is_none());
        crate::events::delete(&mut db, 0).await.unwrap();

        // solvable
        assert!(get_order(&mut db, 3).await.is_some());

        // still solvable because only partially filled
        crate::events::append(
            &mut db,
            &[(
                EventIndex {
                    block_number: 0,
                    log_index: 0,
                },
                Event::Trade(Trade {
                    order_uid: order.uid,
                    sell_amount_including_fee: 5.into(),
                    ..Default::default()
                }),
            )],
        )
        .await
        .unwrap();
        assert!(get_order(&mut db, 3).await.is_some());

        //no longer solvable, if it is a ethflow-order
        //with shorter user_valid_to from the ethflow
        let ethflow_order = EthOrderPlacement {
            uid: order.uid,
            valid_to: 2,
        };
        insert_or_overwrite_ethflow_order(&mut db, &ethflow_order)
            .await
            .unwrap();

        assert!(get_order(&mut db, 3).await.is_none());
        assert!(get_order(&mut db, 2).await.is_some());

        // no longer solvable, if there was also a onchain order
        // placement error
        let onchain_order_placement = OnchainOrderPlacement {
            placement_error: Some(OnchainOrderPlacementError::QuoteNotFound),
            ..Default::default()
        };
        let event_index = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        insert_onchain_order(&mut db, &event_index, &onchain_order_placement)
            .await
            .unwrap();

        assert!(get_order(&mut db, 2).await.is_none());
    }

    type Data = ([u8; 56], Address, DateTime<Utc>);
    async fn user_orders(
        ex: &mut PgConnection,
        owner: &Address,
        offset: i64,
        limit: Option<i64>,
    ) -> Vec<Data> {
        super::user_orders(ex, owner, offset, limit)
            .map(|o| {
                let o = o.unwrap();
                (o.uid.0, o.owner, o.creation_timestamp)
            })
            .collect::<Vec<_>>()
            .await
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_user_orders_performance_many_users_with_some_orders() {
        // The following test can be used as performance test,
        // if the values for i and j are increased ->i=100
        // and j=1000 the query should still 10 ms
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        for i in 0..1u32 {
            let mut owner_bytes = i.to_ne_bytes().to_vec();
            owner_bytes.append(&mut vec![0; 20 - owner_bytes.len()]);
            let owner = ByteArray(owner_bytes.try_into().unwrap());
            for j in 0..10u32 {
                let mut i_as_bytes = i.to_ne_bytes().to_vec();
                let mut j_as_bytes = j.to_ne_bytes().to_vec();
                let mut order_uid_info = vec![0; 56 - i_as_bytes.len() - j_as_bytes.len()];
                order_uid_info.append(&mut j_as_bytes);
                i_as_bytes.append(&mut order_uid_info);
                let uid = ByteArray(i_as_bytes.try_into().unwrap());
                let order = Order {
                    owner,
                    uid,
                    creation_timestamp: Utc::now(),
                    ..Default::default()
                };
                insert_order(&mut db, &order).await.unwrap();
                if j % 10 == 0 {
                    let onchain_order = OnchainOrderPlacement {
                        order_uid: uid,
                        sender: owner,
                        placement_error: None,
                    };
                    let event_index = EventIndex::default();
                    insert_onchain_order(&mut db, &event_index, &onchain_order)
                        .await
                        .unwrap();
                }
            }
        }

        let now = std::time::Instant::now();
        let number_of_query_executions = 100;
        for _ in 0..number_of_query_executions {
            let _result = user_orders(&mut db, &ByteArray([2u8; 20]), 10, Some(10)).await;
        }
        let elapsed = now.elapsed();
        println!(
            "Time per execution {:?}",
            elapsed / number_of_query_executions
        );
        assert!(elapsed / number_of_query_executions < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_user_orders_performance_user_with_many_orders() {
        // The following test can be used as performance test close to prod env,
        // if the values for j increased ->j=100_000 query should still finish
        // below 200 ms
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        for i in 0..1u32 {
            let mut owner_bytes = i.to_ne_bytes().to_vec();
            owner_bytes.append(&mut vec![0; 20 - owner_bytes.len()]);
            let owner = ByteArray(owner_bytes.try_into().unwrap());
            for j in 0..10u32 {
                let mut i_as_bytes = i.to_ne_bytes().to_vec();
                let mut j_as_bytes = j.to_ne_bytes().to_vec();
                let mut order_uid_info = vec![0; 56 - i_as_bytes.len() - j_as_bytes.len()];
                order_uid_info.append(&mut j_as_bytes);
                i_as_bytes.append(&mut order_uid_info);
                let order = Order {
                    owner,
                    uid: ByteArray(i_as_bytes.try_into().unwrap()),
                    creation_timestamp: Utc::now(),
                    ..Default::default()
                };
                insert_order(&mut db, &order).await.unwrap();
            }
        }

        let now = std::time::Instant::now();
        let number_of_query_executions = 100;
        for _ in 0..number_of_query_executions {
            let _result = user_orders(&mut db, &ByteArray([0u8; 20]), 10, Some(10)).await;
        }
        let elapsed = now.elapsed();
        println!(
            "Time per execution {:?}",
            elapsed / number_of_query_executions
        );
        assert!(elapsed / number_of_query_executions < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_user_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let owners: Vec<Address> = (0u8..3).map(|i| ByteArray([i; 20])).collect();

        fn datetime(offset: u32) -> DateTime<Utc> {
            Utc.timestamp_opt(offset as i64, 0).unwrap()
        }

        type Data = ([u8; 56], Address, DateTime<Utc>);
        let orders = [
            ([3u8; 56], owners[0], datetime(3)),
            ([1u8; 56], owners[1], datetime(2)),
            ([0u8; 56], owners[0], datetime(1)),
            ([2u8; 56], owners[1], datetime(0)),
        ];

        for order in &orders {
            let order = Order {
                uid: ByteArray(order.0),
                owner: order.1,
                creation_timestamp: order.2,
                ..Default::default()
            };
            insert_order(&mut db, &order).await.unwrap();
        }

        async fn user_orders(
            ex: &mut PgConnection,
            owner: &Address,
            offset: i64,
            limit: Option<i64>,
        ) -> Vec<Data> {
            super::user_orders(ex, owner, offset, limit)
                .map(|o| {
                    let o = o.unwrap();
                    (o.uid.0, o.owner, o.creation_timestamp)
                })
                .collect::<Vec<_>>()
                .await
        }

        let result = user_orders(&mut db, &owners[0], 0, None).await;
        assert_eq!(result, vec![orders[0], orders[2]]);

        let result = user_orders(&mut db, &owners[1], 0, None).await;
        assert_eq!(result, vec![orders[1], orders[3]]);

        let result = user_orders(&mut db, &owners[0], 0, Some(1)).await;
        assert_eq!(result, vec![orders[0]]);

        let result = user_orders(&mut db, &owners[0], 1, Some(1)).await;
        assert_eq!(result, vec![orders[2]]);

        let result = user_orders(&mut db, &owners[0], 2, Some(1)).await;
        assert_eq!(result, vec![]);

        let onchain_order = OnchainOrderPlacement {
            order_uid: ByteArray(orders[0].0),
            sender: owners[2],
            placement_error: None,
        };
        let event_index = EventIndex::default();
        insert_onchain_order(&mut db, &event_index, &onchain_order)
            .await
            .unwrap();
        let result = user_orders(&mut db, &owners[2], 0, Some(1)).await;
        assert_eq!(result, vec![orders[0]]);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_orders_in_tx() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let uid = |i: u8| ByteArray([i; 56]);
        let tx_hash = |i: u8| ByteArray([i; 32]);
        let uid_to_order = |uid: &OrderUid| Order {
            uid: *uid,
            ..Default::default()
        };
        let trade = |block_number, log_index, order_uid| {
            (
                EventIndex {
                    block_number,
                    log_index,
                },
                Event::Trade(Trade {
                    order_uid,
                    ..Default::default()
                }),
            )
        };
        let settlement = |block_number, log_index, transaction_hash| {
            (
                EventIndex {
                    block_number,
                    log_index,
                },
                Event::Settlement(Settlement {
                    transaction_hash,
                    ..Default::default()
                }),
            )
        };

        for i in 0..8 {
            insert_order(&mut db, &uid_to_order(&uid(i))).await.unwrap();
        }
        let events = &[
            // first block, 1 settlement, 1 order
            trade(0, 0, uid(0)),
            settlement(0, 1, tx_hash(0)),
            // second block, 3 settlements with 2 orders each
            trade(1, 0, uid(1)),
            trade(1, 1, uid(2)),
            settlement(1, 2, tx_hash(1)),
            trade(1, 3, uid(3)),
            trade(1, 4, uid(4)),
            settlement(1, 5, tx_hash(2)),
            trade(1, 6, uid(5)),
            trade(1, 7, uid(6)),
            settlement(1, 8, tx_hash(3)),
            // third block, 1 settlement, 1 order
            trade(2, 0, uid(7)),
            settlement(2, 1, tx_hash(4)),
        ];
        crate::events::append(&mut db, events).await.unwrap();

        for (tx_hash, expected_uids) in [
            (tx_hash(0), &[uid(0)] as &[OrderUid]),
            (tx_hash(1), &[uid(1), uid(2)]),
            (tx_hash(2), &[uid(3), uid(4)]),
            (tx_hash(3), &[uid(5), uid(6)]),
            (tx_hash(4), &[uid(7)]),
        ] {
            let actual = full_orders_in_tx(&mut db, &tx_hash)
                .map_ok(|order| order.uid)
                .try_collect::<Vec<_>>()
                .await
                .unwrap();
            assert_eq!(actual, expected_uids);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_latest_settlement_block() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        assert_eq!(latest_settlement_block(&mut db).await.unwrap(), 0);
        let event = (
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Event::Settlement(Default::default()),
        );
        crate::events::append(&mut db, &[event]).await.unwrap();
        assert_eq!(latest_settlement_block(&mut db).await.unwrap(), 0);
        let event = (
            EventIndex {
                block_number: 3,
                log_index: 0,
            },
            Event::Settlement(Default::default()),
        );
        crate::events::append(&mut db, &[event]).await.unwrap();
        assert_eq!(latest_settlement_block(&mut db).await.unwrap(), 3);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_update_multiple_identical_limit_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        for id in 1..3 {
            insert_order(
                &mut db,
                &Order {
                    uid: ByteArray([id; 56]),
                    sell_token: ByteArray([1; 20]),
                    buy_token: ByteArray([2; 20]),
                    sell_amount: 1_000.into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        }

        let order_spec = OrderFeeSpecifier {
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: 1_000.into(),
        };
        let update = FeeUpdate {
            surplus_fee: Some(42.into()),
            surplus_fee_timestamp: Utc.timestamp_opt(1234567890, 0).unwrap(),
            full_fee_amount: 1337.into(),
        };
        let updated_uids = update_fok_limit_order_fees(&mut db, &order_spec, &update)
            .await
            .unwrap();
        assert_eq!(updated_uids, vec![ByteArray([1; 56]), ByteArray([2; 56])]);

        for id in 1..3 {
            let order = read_order(&mut db, &ByteArray([id; 56]))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(order.surplus_fee, update.surplus_fee);
            assert_eq!(
                order.surplus_fee_timestamp,
                Some(update.surplus_fee_timestamp)
            );
            assert_eq!(order.full_fee_amount, update.full_fee_amount);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_limit_orders_with_outdated_fees() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let timestamp = Utc.timestamp_opt(1234567890, 0).unwrap();
        // Valid limit order with an outdated surplus fee.
        let order = Order {
            uid: ByteArray([1; 56]),
            class: OrderClass::Limit,
            valid_to: 3,
            surplus_fee: Some(0.into()),
            surplus_fee_timestamp: Some(timestamp - chrono::Duration::seconds(1)),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: 1.into(),
            buy_amount: 1.into(),
            ..Default::default()
        };
        insert_order(&mut db, &order).await.unwrap();

        // Like previous order but partially fillable so shouldn't get included.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([8; 56]),
                partially_fillable: true,
                ..order
            },
        )
        .await
        .unwrap();

        // Give the order a pre-interaction to test that the query finds it.
        insert_or_overwrite_pre_interaction(&mut db, 0, &Default::default(), &ByteArray([1; 56]))
            .await
            .unwrap();

        // Expired limit order.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([2; 56]),
                class: OrderClass::Limit,
                valid_to: 1,
                surplus_fee: Some(0.into()),
                surplus_fee_timestamp: Some(Default::default()),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Cancelled limit order.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([3; 56]),
                class: OrderClass::Limit,
                valid_to: 1,
                cancellation_timestamp: Some(Utc::now()),
                surplus_fee: Some(0.into()),
                surplus_fee_timestamp: Some(Default::default()),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Limit order with a recent surplus fee timestamp.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([4; 56]),
                class: OrderClass::Limit,
                valid_to: 3,
                surplus_fee: Some(0.into()),
                surplus_fee_timestamp: Some(timestamp),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Limit order that was never estimated.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([5; 56]),
                class: OrderClass::Limit,
                valid_to: 3,
                surplus_fee: None,
                surplus_fee_timestamp: None,
                sell_token: ByteArray([3; 20]),
                buy_token: ByteArray([4; 20]),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Limit order that was recently unsuccessfully estimated.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([6; 56]),
                class: OrderClass::Limit,
                valid_to: 3,
                surplus_fee: None,
                surplus_fee_timestamp: Some(timestamp),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Not a limit order.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([7; 56]),
                valid_to: 3,
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let orders: Vec<_> = open_fok_limit_orders(&mut db, timestamp, 2)
            .try_collect()
            .await
            .unwrap();

        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].uid, ByteArray([5; 56]));
        assert_eq!(orders[1].uid, ByteArray([1; 56]),);
        assert_eq!(orders[1].pre_interactions, 1);

        // Invalidate one of the orders through a trade.
        crate::events::insert_trade(
            &mut db,
            &EventIndex::default(),
            &Trade {
                order_uid: ByteArray([1; 56]),
                sell_amount_including_fee: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let orders: Vec<_> = open_fok_limit_orders(&mut db, timestamp, 2)
            .try_collect()
            .await
            .unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].uid, ByteArray([5; 56]));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_limit_order_executed() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_uid = ByteArray([1; 56]);
        insert_order(
            &mut db,
            &Order {
                uid: order_uid,
                class: OrderClass::Limit,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let order = single_full_order(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(order.executed_surplus_fee, None);

        let fee: BigDecimal = 1.into();
        let solver_fee: BigDecimal = 2.into();
        crate::order_execution::save(&mut db, &order_uid, 0, Some(&fee), &solver_fee)
            .await
            .unwrap();

        let order = single_full_order(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(order.executed_surplus_fee, Some(fee));
        assert_eq!(order.executed_solver_fee, Some(solver_fee));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_limit_order_counting() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let timestamp = Utc.timestamp_opt(1234567890, 0).unwrap();
        let order_uid = ByteArray([1; 56]);
        // Valid limit order with an outdated surplus fee.
        insert_order(
            &mut db,
            &Order {
                uid: order_uid,
                class: OrderClass::Limit,
                valid_to: 3,
                surplus_fee: Some(0.into()),
                surplus_fee_timestamp: Some(Default::default()),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Expired limit order.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([2; 56]),
                class: OrderClass::Limit,
                valid_to: 1,
                surplus_fee: Some(0.into()),
                surplus_fee_timestamp: Some(Default::default()),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Cancelled limit order.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([3; 56]),
                class: OrderClass::Limit,
                valid_to: 1,
                cancellation_timestamp: Some(Utc::now()),
                surplus_fee: Some(0.into()),
                surplus_fee_timestamp: Some(Default::default()),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Limit order with a recent surplus fee timestamp.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([4; 56]),
                class: OrderClass::Limit,
                valid_to: 3,
                surplus_fee: Some(0.into()),
                surplus_fee_timestamp: Some(timestamp),
                owner: ByteArray([7u8; 20]),
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Not a limit order.
        insert_order(
            &mut db,
            &Order {
                uid: ByteArray([5; 56]),
                valid_to: 3,
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(count_fok_limit_orders(&mut db, 2).await.unwrap(), 2);
        assert_eq!(
            count_fok_limit_orders_with_outdated_fees(&mut db, timestamp, 2)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            count_limit_orders_by_owner(&mut db, 2, &ByteArray([7u8; 20]))
                .await
                .unwrap(),
            1
        );
    }
}

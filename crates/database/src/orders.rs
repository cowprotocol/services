use crate::{Address, AppId, OrderUid, TransactionHash};
use futures::stream::BoxStream;
use sqlx::{
    types::{
        chrono::{DateTime, NaiveDateTime, Utc},
        BigDecimal,
    },
    PgConnection,
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

/// Destination for which the buyAmount should be transferred to order's receiver to upon fulfilment
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

/// One row in the `orders` table.
#[derive(Clone, Debug, Eq, PartialEq, sqlx::FromRow)]
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
    pub is_liquidity_order: bool,
    pub cancellation_timestamp: Option<DateTime<Utc>>,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            uid: Default::default(),
            owner: Default::default(),
            creation_timestamp: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            sell_token: Default::default(),
            buy_token: Default::default(),
            receiver: Default::default(),
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            valid_to: Default::default(),
            app_data: Default::default(),
            fee_amount: Default::default(),
            kind: Default::default(),
            partially_fillable: Default::default(),
            signature: Default::default(),
            signing_scheme: Default::default(),
            settlement_contract: Default::default(),
            sell_token_balance: Default::default(),
            buy_token_balance: Default::default(),
            full_fee_amount: Default::default(),
            is_liquidity_order: Default::default(),
            cancellation_timestamp: Default::default(),
        }
    }
}

pub async fn insert_orders_and_ignore_conflicts(
    ex: &mut PgConnection,
    orders: &[Order],
) -> Result<(), sqlx::Error> {
    for order in orders {
        match insert_order(ex, order).await {
            Ok(_) => (),
            Err(err) if is_duplicate_record_error(&err) => (),
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

pub async fn insert_order(ex: &mut PgConnection, order: &Order) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
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
    is_liquidity_order,
    cancellation_timestamp
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
    "#;
    sqlx::query(QUERY)
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
        .bind(order.is_liquidity_order)
        .bind(order.cancellation_timestamp)
        .execute(ex)
        .await?;
    Ok(())
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

pub async fn insert_quote(ex: &mut PgConnection, quote: &Quote) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO order_quotes (
    order_uid,
    gas_amount,
    gas_price,
    sell_token_price,
    sell_amount,
    buy_amount
)
VALUES ($1, $2, $3, $4, $5, $6)
    "#;
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

/// Order with extra information from other tables. Has all the information needed to construct a model::Order.
#[derive(sqlx::FromRow)]
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
    pub is_liquidity_order: bool,
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
const ORDERS_SELECT: &str = r#"
o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, o.buy_amount,
o.valid_to, o.app_data, o.fee_amount, o.full_fee_amount, o.kind, o.partially_fillable, o.signature,
o.receiver, o.signing_scheme, o.settlement_contract, o.sell_token_balance, o.buy_token_balance,
o.is_liquidity_order,
(SELECT COALESCE(SUM(t.buy_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_buy,
(SELECT COALESCE(SUM(t.sell_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_sell,
(SELECT COALESCE(SUM(t.fee_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_fee,
(o.cancellation_timestamp IS NOT NULL OR
    (SELECT COUNT(*) FROM invalidations WHERE invalidations.order_uid = o.uid) > 0
) AS invalidated,
(o.signing_scheme = 'presign' AND COALESCE((
    SELECT (NOT p.signed) as unsigned
    FROM presignature_events p
    WHERE o.uid = p.order_uid
    ORDER BY p.block_number DESC, p.log_index DESC
    LIMIT 1
), true)) AS presignature_pending
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

pub fn full_orders_in_tx<'a>(
    ex: &'a mut PgConnection,
    tx_hash: &'a TransactionHash,
) -> BoxStream<'a, Result<FullOrder, sqlx::Error>> {
    const QUERY: &str = const_format::formatcp!(
        r#"
-- This will fail if we ever have multiple settlements in the same transaction because this WITH
-- query will return multiple rows. If we ever want to do this, a tx hash is no longer enough to
-- uniquely identify a settlement so the "orders for tx hash" route needs to change in some way
-- like taking block number and log index directly.
WITH settlement as (
    SELECT block_number, log_index
    FROM settlements
    WHERE tx_hash = $1
)
SELECT {ORDERS_SELECT}
FROM {ORDERS_FROM}
JOIN trades t ON t.order_uid = o.uid
WHERE
    t.block_number = (SELECT block_number FROM settlement) AND
    -- BETWEEN is inclusive
    t.log_index BETWEEN
        -- previous settlement, the settlement event is emitted after the trade events
        (
            -- COALESCE because there might not be a previous settlement
            SELECT COALESCE(MAX(log_index), 0)
            FROM settlements
            WHERE
                block_number = (SELECT block_number FROM settlement) AND
                log_index < (SELECT log_index from settlement)
        ) AND
        (SELECT log_index FROM settlement)
;"#
    );
    sqlx::query_as(QUERY).bind(tx_hash).fetch(ex)
}

pub fn user_orders<'a>(
    ex: &'a mut PgConnection,
    owner: &'a Address,
    offset: i64,
    limit: Option<i64>,
) -> BoxStream<'a, Result<FullOrder, sqlx::Error>> {
    // As a future consideration for this query we could move from offset to an approach called
    // keyset pagination where the offset is identified by "key" of the previous query. In our
    // case that would be the lowest creation_timestamp. This way the database can start
    // immediately at the offset through the index without enumerating the first N elements
    // before as is the case with OFFSET.
    // On the other hand that approach is less flexible so we will consider if we see that these
    // queries are taking too long in practice.
    #[rustfmt::skip]
    const QUERY: &str = const_format::concatcp!(
"SELECT ", ORDERS_SELECT,
" FROM ", ORDERS_FROM,
" WHERE o.owner = $1 ",
"ORDER BY o.creation_timestamp DESC ",
"LIMIT $2 ",
"OFFSET $3 ",
    );
    sqlx::query_as(QUERY)
        .bind(owner)
        .bind(limit)
        .bind(offset)
        .fetch(ex)
}

pub fn solvable_orders(
    ex: &mut PgConnection,
    min_valid_to: i64,
) -> BoxStream<'_, Result<FullOrder, sqlx::Error>> {
    #[rustfmt::skip]
    const QUERY: &str = const_format::concatcp!(
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
    (NOT presignature_pending);
"#
    );
    sqlx::query_as(QUERY).bind(min_valid_to).fetch(ex)
}

pub async fn latest_settlement_block(ex: &mut PgConnection) -> Result<i64, sqlx::Error> {
    const QUERY: &str = r#"
SELECT COALESCE(MAX(block_number), 0)
FROM settlements
    "#;
    sqlx::query_scalar(QUERY).fetch_one(ex).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        byte_array::ByteArray,
        ethflow_orders::{insert_ethflow_order, EthOrderPlacement},
        events::{Event, EventIndex, Invalidation, PreSignature, Settlement, Trade},
        PgTransaction,
    };
    use bigdecimal::num_bigint::{BigInt, ToBigInt};
    use futures::{StreamExt, TryStreamExt};
    use sqlx::Connection;

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

        let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(1234567890, 0), Utc);
        cancel_order(&mut db, &order.uid, time).await.unwrap();
        let order = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert_eq!(time, order.cancellation_timestamp.unwrap());

        // Cancel again and verify that cancellation timestamp was not changed.
        let irrelevant_time = DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp(1234567890, 1_000_000_000),
            Utc,
        );
        assert_ne!(irrelevant_time, time);
        cancel_order(&mut db, &order.uid, time).await.unwrap();
        let order = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert_eq!(time, order.cancellation_timestamp.unwrap());
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
            solvable_orders(ex, 0).next().await.transpose().unwrap()
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
            solvable_orders(ex, min_valid_to)
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
            is_refunded: false,
        };
        insert_ethflow_order(&mut db, &ethflow_order).await.unwrap();

        assert!(get_order(&mut db, 3).await.is_none());
        assert!(get_order(&mut db, 2).await.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_user_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let owners: Vec<Address> = (0u8..2).map(|i| ByteArray([i; 20])).collect();

        fn datetime(offset: u32) -> DateTime<Utc> {
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(offset as i64, 0), Utc)
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
}

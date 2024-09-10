use {
    crate::{
        orders::{self, BuyTokenDestination, OrderKind, SellTokenSource, SigningScheme},
        Address,
        AppId,
        OrderUid,
        TransactionHash,
    },
    sqlx::{
        types::{
            chrono::{DateTime, Utc},
            BigDecimal,
        },
        PgConnection,
        QueryBuilder,
    },
};

pub const SELECT: &str = r#"
o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, o.buy_amount,
o.valid_to, o.app_data, o.fee_amount, o.fee_amount AS full_fee_amount, o.kind, o.partially_fillable, o.signature,
o.receiver, o.signing_scheme, '\x9008d19f58aabd9ed0d60971565aa8510560ab41'::bytea AS settlement_contract, o.sell_token_balance, o.buy_token_balance,
'liquidity'::OrderClass AS class,
(SELECT COALESCE(SUM(t.buy_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_buy,
(SELECT COALESCE(SUM(t.sell_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_sell,
(SELECT COALESCE(SUM(t.fee_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_fee,
FALSE AS invalidated,
FALSE AS presignature_pending,
ARRAY[]::record[] AS pre_interactions,
ARRAY[]::record[] AS post_interactions,
NULL AS ethflow_data,
NULL AS onchain_user,
NULL AS onchain_placement_error,
COALESCE((SELECT SUM(surplus_fee) FROM order_execution oe WHERE oe.order_uid = o.uid), 0) as executed_surplus_fee,
COALESCE((SELECT surplus_fee_token FROM order_execution oe WHERE oe.order_uid = o.uid), o.sell_token) as executed_surplus_fee_token, -- TODO surplus token
NULL AS full_app_data
"#;

pub const FROM: &str = "jit_orders o";

pub async fn get_by_id(
    ex: &mut PgConnection,
    uid: &OrderUid,
) -> Result<Option<orders::FullOrder>, sqlx::Error> {
    #[rustfmt::skip]
        const QUERY: &str = const_format::concatcp!(
"SELECT ",
SELECT,
" FROM ", FROM,
" WHERE o.uid = $1 ",
        );
    sqlx::query_as(QUERY).bind(uid).fetch_optional(ex).await
}

pub async fn get_by_tx(
    ex: &mut PgConnection,
    tx_hash: &TransactionHash,
) -> Result<Vec<orders::FullOrder>, sqlx::Error> {
    const QUERY: &str = const_format::concatcp!(
        orders::SETTLEMENT_LOG_INDICES,
        "SELECT ",
        SELECT,
        " FROM ",
        FROM,
        " JOIN trades t ON t.order_uid = o.uid",
        " WHERE
        t.block_number = (SELECT block_number FROM settlement) AND
        -- BETWEEN is inclusive
        t.log_index BETWEEN (SELECT * from previous_settlement) AND (SELECT log_index FROM \
         settlement) 
        AND NOT EXISTS (
            SELECT 1 FROM orders ord
            WHERE ord.uid = o.uid)
        ",
    );
    sqlx::query_as(QUERY).bind(tx_hash).fetch_all(ex).await
}

/// 1:1 mapping to the `jit_orders` table, used to store orders.
#[derive(Debug, Clone, Default, PartialEq, sqlx::FromRow)]
pub struct JitOrder {
    pub block_number: i64,
    pub log_index: i64,
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
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub signature: Vec<u8>,
    pub receiver: Address,
    pub signing_scheme: SigningScheme,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
}

pub async fn insert(ex: &mut PgConnection, jit_orders: &[JitOrder]) -> Result<(), sqlx::Error> {
    if jit_orders.is_empty() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::new(
        r#"
        INSERT INTO jit_orders (
            block_number,
            log_index,
            uid,
            owner,
            creation_timestamp,
            sell_token,
            buy_token,
            sell_amount,
            buy_amount,
            valid_to,
            app_data,
            fee_amount,
            kind,
            partially_fillable,
            signature,
            receiver,
            signing_scheme,
            sell_token_balance,
            buy_token_balance
        ) 
        "#,
    );

    query_builder.push_values(jit_orders.iter(), |mut builder, jit_order| {
        builder
            .push_bind(jit_order.block_number)
            .push_bind(jit_order.log_index)
            .push_bind(jit_order.uid)
            .push_bind(jit_order.owner)
            .push_bind(jit_order.creation_timestamp)
            .push_bind(jit_order.sell_token)
            .push_bind(jit_order.buy_token)
            .push_bind(jit_order.sell_amount.clone())
            .push_bind(jit_order.buy_amount.clone())
            .push_bind(jit_order.valid_to)
            .push_bind(jit_order.app_data)
            .push_bind(jit_order.fee_amount.clone())
            .push_bind(jit_order.kind)
            .push_bind(jit_order.partially_fillable)
            .push_bind(jit_order.signature.clone())
            .push_bind(jit_order.receiver)
            .push_bind(jit_order.signing_scheme)
            .push_bind(jit_order.sell_token_balance)
            .push_bind(jit_order.buy_token_balance);
    });

    query_builder.push(
        r#"
        ON CONFLICT DO NOTHING"#,
    );

    let query = query_builder.build();
    query.execute(ex).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    pub async fn read_order(
        ex: &mut PgConnection,
        uid: &OrderUid,
    ) -> Result<Option<JitOrder>, sqlx::Error> {
        const QUERY: &str = r#"
    SELECT *
    FROM jit_orders
    WHERE uid = $1
        ;"#;
        sqlx::query_as(QUERY).bind(uid).fetch_optional(ex).await
    }

    use {
        super::*,
        crate::byte_array::ByteArray,
        sqlx::{Connection, PgConnection},
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let jit_order = JitOrder::default();

        // insert a jit order and read it back
        insert(&mut db, &[jit_order.clone()]).await.unwrap();
        let read_jit_order = read_order(&mut db, &jit_order.uid).await.unwrap().unwrap();
        assert_eq!(jit_order, read_jit_order);

        // try to insert updated order, but no update was done on conflict
        let jit_order_updated = JitOrder {
            creation_timestamp: DateTime::<Utc>::default() + chrono::Duration::days(1),
            ..jit_order.clone()
        };
        insert(&mut db, &[jit_order_updated.clone()]).await.unwrap();
        let read_jit_order = read_order(&mut db, &jit_order_updated.uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(jit_order, read_jit_order);

        // read non existent order
        let read_jit_order = read_order(&mut db, &ByteArray([1u8; 56])).await.unwrap();
        assert!(read_jit_order.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_get_by_id() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let jit_order = JitOrder::default();

        // insert a jit order and make sure "SELECT" query works properly
        insert(&mut db, &[jit_order.clone()]).await.unwrap();
        get_by_id(&mut db, &jit_order.uid).await.unwrap().unwrap();
    }
}

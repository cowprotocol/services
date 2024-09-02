use {
    crate::{
        orders,
        orders::{BuyTokenDestination, OrderKind, SellTokenSource, SigningScheme},
        Address,
        AppId,
        OrderUid,
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

pub async fn single_full_jit_order(
    ex: &mut PgConnection,
    uid: &OrderUid,
) -> Result<Option<FullJitOrder>, sqlx::Error> {
    #[rustfmt::skip]
        const QUERY: &str = const_format::concatcp!(
"SELECT o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, o.buy_amount,
o.valid_to, o.app_data, o.fee_amount, o.kind, o.partially_fillable, o.signature,
o.receiver, o.signing_scheme, o.sell_token_balance, o.buy_token_balance,
(SELECT COALESCE(SUM(t.buy_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_buy,
(SELECT COALESCE(SUM(t.sell_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_sell,
(SELECT COALESCE(SUM(t.fee_amount), 0) FROM trades t WHERE t.order_uid = o.uid) AS sum_fee,
COALESCE((SELECT SUM(surplus_fee) FROM order_execution oe WHERE oe.order_uid = o.uid), 0) as executed_surplus_fee",
" FROM jit_orders o",
" WHERE o.uid = $1 ",
        );
    sqlx::query_as(QUERY).bind(uid).fetch_optional(ex).await
}

/// Jit order combined with trades table and order_execution table, suitable for
/// API responses.
#[derive(Debug, Clone, Default, PartialEq, sqlx::FromRow)]
pub struct FullJitOrder {
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
    pub sum_sell: BigDecimal,
    pub sum_buy: BigDecimal,
    pub sum_fee: BigDecimal,
    pub receiver: Address,
    pub signing_scheme: SigningScheme,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub executed_surplus_fee: BigDecimal,
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

pub async fn upsert_orders(
    ex: &mut PgConnection,
    jit_orders: &[JitOrder],
) -> Result<(), sqlx::Error> {
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
        ON CONFLICT (block_number, log_index) DO UPDATE 
        SET uid = EXCLUDED.uid,
            owner = EXCLUDED.owner,
            creation_timestamp = EXCLUDED.creation_timestamp,
            sell_token = EXCLUDED.sell_token,
            buy_token = EXCLUDED.buy_token,
            sell_amount = EXCLUDED.sell_amount,
            buy_amount = EXCLUDED.buy_amount,
            valid_to = EXCLUDED.valid_to,
            app_data = EXCLUDED.app_data,
            fee_amount = EXCLUDED.fee_amount,
            kind = EXCLUDED.kind,
            partially_fillable = EXCLUDED.partially_fillable,
            signature = EXCLUDED.signature,
            receiver = EXCLUDED.receiver,
            signing_scheme = EXCLUDED.signing_scheme,
            sell_token_balance = EXCLUDED.sell_token_balance,
            buy_token_balance = EXCLUDED.buy_token_balance;
        "#,
    );

    let query = query_builder.build();
    query.execute(ex).await?;

    Ok(())
}

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

impl From<FullJitOrder> for orders::FullOrder {
    fn from(jit_order: FullJitOrder) -> Self {
        orders::FullOrder {
            uid: jit_order.uid,
            owner: jit_order.owner,
            creation_timestamp: jit_order.creation_timestamp,
            sell_token: jit_order.sell_token,
            buy_token: jit_order.buy_token,
            sell_amount: jit_order.sell_amount,
            buy_amount: jit_order.buy_amount,
            valid_to: jit_order.valid_to,
            app_data: jit_order.app_data,
            fee_amount: jit_order.fee_amount.clone(),
            full_fee_amount: jit_order.fee_amount,
            kind: jit_order.kind,
            class: orders::OrderClass::Limit, // irrelevant
            partially_fillable: jit_order.partially_fillable,
            signature: jit_order.signature,
            sum_sell: jit_order.sum_sell,
            sum_buy: jit_order.sum_buy,
            sum_fee: jit_order.sum_fee,
            invalidated: false,
            receiver: Some(jit_order.receiver),
            signing_scheme: jit_order.signing_scheme,
            settlement_contract: Address::default(),
            sell_token_balance: jit_order.sell_token_balance,
            buy_token_balance: jit_order.buy_token_balance,
            presignature_pending: false,
            pre_interactions: Vec::new(),
            post_interactions: Vec::new(),
            ethflow_data: None,
            onchain_user: None,
            onchain_placement_error: None,
            executed_surplus_fee: jit_order.executed_surplus_fee,
            full_app_data: None,
        }
    }
}

#[cfg(test)]
mod tests {
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

        // insert a jit order
        let jit_order = JitOrder {
            ..Default::default()
        };

        upsert_orders(&mut db, &[jit_order.clone()]).await.unwrap();

        // read it back
        let jit_order2 = read_order(&mut db, &jit_order.uid).await.unwrap().unwrap();
        assert_eq!(jit_order, jit_order2);

        // read non existent order
        let jit_order3 = read_order(&mut db, &ByteArray([1u8; 56])).await.unwrap();
        assert!(jit_order3.is_none());
    }
}

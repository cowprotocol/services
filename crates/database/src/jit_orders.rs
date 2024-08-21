use {
    crate::{
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
    },
};

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
    pub signature: Vec<u8>,
    pub receiver: Address,
    pub signing_scheme: SigningScheme,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
}

pub async fn upsert_order(ex: &mut PgConnection, jit_order: JitOrder) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
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
        signature,
        receiver,
        signing_scheme,
        sell_token_balance,
        buy_token_balance
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
    ON CONFLICT (block_number, log_index) DO UPDATE 
SET uid = $3, owner = $4, creation_timestamp = $5, sell_token = $6, buy_token = $7, sell_amount = $8, buy_amount = $9, valid_to = $10, app_data = $11, fee_amount = $12, kind = $13, signature = $14, receiver = $15, signing_scheme = $16, sell_token_balance = $17, buy_token_balance = $18
    ;"#;
    sqlx::query(QUERY)
        .bind(jit_order.block_number)
        .bind(jit_order.log_index)
        .bind(jit_order.uid)
        .bind(jit_order.owner)
        .bind(jit_order.creation_timestamp)
        .bind(jit_order.sell_token)
        .bind(jit_order.buy_token)
        .bind(jit_order.sell_amount)
        .bind(jit_order.buy_amount)
        .bind(jit_order.valid_to)
        .bind(jit_order.app_data)
        .bind(jit_order.fee_amount)
        .bind(jit_order.kind)
        .bind(jit_order.signature)
        .bind(jit_order.receiver)
        .bind(jit_order.signing_scheme)
        .bind(jit_order.sell_token_balance)
        .bind(jit_order.buy_token_balance)
        .execute(ex)
        .await?;
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

        upsert_order(&mut db, jit_order.clone()).await.unwrap();

        // read it back
        let jit_order2 = read_order(&mut db, &jit_order.uid).await.unwrap().unwrap();
        assert_eq!(jit_order, jit_order2);

        // read non existent order
        let jit_order3 = read_order(&mut db, &ByteArray([1u8; 56])).await.unwrap();
        assert!(jit_order3.is_none());
    }
}

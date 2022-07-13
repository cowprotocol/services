use crate::{Address, AppId, OrderUid};
use sqlx::{
    types::{
        chrono::{DateTime, NaiveDateTime, Utc},
        BigDecimal,
    },
    PgConnection,
};

#[derive(Clone, Copy, Default, sqlx::Type)]
#[sqlx(type_name = "OrderKind")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderKind {
    #[default]
    Buy,
    Sell,
}

#[derive(Clone, Copy, Default, PartialEq, sqlx::Type)]
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
#[derive(Clone, Copy, Default, sqlx::Type)]
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
#[derive(Clone, Copy, Default, sqlx::Type)]
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
#[derive(Clone, sqlx::FromRow)]
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
        }
    }
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
    is_liquidity_order
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
    "#;
    sqlx::query(QUERY)
        .bind(order.uid)
        .bind(order.owner)
        .bind(order.creation_timestamp)
        .bind(order.sell_token)
        .bind(order.buy_token)
        .bind(order.receiver)
        .bind(&order.sell_amount)
        .bind(&order.buy_amount)
        .bind(order.valid_to)
        .bind(order.app_data)
        .bind(&order.fee_amount)
        .bind(&order.kind)
        .bind(order.partially_fillable)
        .bind(order.signature.as_slice())
        .bind(order.signing_scheme)
        .bind(order.settlement_contract)
        .bind(order.sell_token_balance)
        .bind(order.buy_token_balance)
        .bind(&order.full_fee_amount)
        .bind(order.is_liquidity_order)
        .execute(ex)
        .await?;
    Ok(())
}

pub fn is_duplicate_record_error(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_err) = &err {
        if let Some(code) = db_err.code() {
            return code.as_ref() == "23505";
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{Connection, PgConnection};

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
}

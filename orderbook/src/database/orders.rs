use super::Database;
use crate::u256_conversions::*;
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

    // TODO: add filters: not_fully_executed, owner, sell_token, buy_token
    pub fn orders(&self, min_valid_to: u32) -> impl Stream<Item = Result<Order>> + '_ {
        // TODO: adapt query to filter fully executed orders immediately
        const QUERY: &str = "\
            SELECT \
                o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, o.sell_amount, \
                o.buy_amount, o.valid_to, o.app_data, o.fee_amount, o.kind, o.partially_fillable, \
                o.signature, \
                COALESCE(SUM(t.sell_amount), 0) AS sum_sell, \
                COALESCE(SUM(t.buy_amount), 0) AS sum_buy, \
                COALESCE(SUM(t.fee_amount), 0) AS sum_fee \
            FROM orders o LEFT OUTER JOIN trades t ON o.uid = t.order_uid
            WHERE o.valid_to >= $1 \
            GROUP BY o.uid;";
        // To get only not fully executed orders we probably want to use something like
        // `HAVING sum_sell < orders.sell_amount` but still need to find the best way to make this
        // pick the correct column based on order type.
        sqlx::query_as(QUERY)
            .bind(min_valid_to)
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
    // TODO: add to OrderMetaData
    #[allow(dead_code)]
    sum_sell: BigDecimal,
    #[allow(dead_code)]
    sum_buy: BigDecimal,
    #[allow(dead_code)]
    sum_fee: BigDecimal,
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
    use super::*;
    use chrono::NaiveDateTime;
    use futures::StreamExt;
    use primitive_types::U256;

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
        assert!(db.orders(0).boxed().next().await.is_none());
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
            db.orders(0).try_collect::<Vec<Order>>().await.unwrap(),
            vec![order]
        );
    }
}

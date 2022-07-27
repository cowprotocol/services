use crate::{orders::OrderKind, Address};
use bigdecimal::BigDecimal;
use sqlx::{
    types::chrono::{DateTime, Utc},
    PgConnection,
};

pub type QuoteId = i64;

/// One row in the `quotes` table.
#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Quote {
    pub id: QuoteId,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub gas_amount: f64,
    pub gas_price: f64,
    pub sell_token_price: f64,
    pub order_kind: OrderKind,
    pub expiration_timestamp: DateTime<Utc>,
}

/// Stores the quote and returns the id. The id of the quote parameter is not used.
pub async fn save(ex: &mut PgConnection, quote: &Quote) -> Result<QuoteId, sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO quotes (
    sell_token,
    buy_token,
    sell_amount,
    buy_amount,
    gas_amount,
    gas_price,
    sell_token_price,
    order_kind,
    expiration_timestamp
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
RETURNING id
    "#;
    let (id,) = sqlx::query_as(QUERY)
        .bind(&quote.sell_token)
        .bind(&quote.buy_token)
        .bind(&quote.sell_amount)
        .bind(&quote.buy_amount)
        .bind(quote.gas_amount)
        .bind(quote.gas_price)
        .bind(quote.sell_token_price)
        .bind(quote.order_kind)
        .bind(quote.expiration_timestamp)
        .fetch_one(ex)
        .await?;
    Ok(id)
}

pub async fn get(ex: &mut PgConnection, id: QuoteId) -> Result<Option<Quote>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT *
FROM quotes
WHERE id = $1
    "#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

/// Fields for searching stored quotes.
#[derive(Clone)]
pub struct QuoteSearchParameters {
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount_0: BigDecimal,
    pub sell_amount_1: BigDecimal,
    pub buy_amount: BigDecimal,
    pub kind: OrderKind,
    pub expiration: DateTime<Utc>,
}

pub async fn find(
    ex: &mut PgConnection,
    params: &QuoteSearchParameters,
) -> Result<Option<Quote>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT *
FROM quotes
WHERE
    sell_token = $1 AND
    buy_token = $2 AND
    (
        (order_kind = 'sell' AND sell_amount = $3) OR
        (order_kind = 'sell' AND sell_amount = $4) OR
        (order_kind = 'buy' AND buy_amount = $5)
    ) AND
    order_kind = $6 AND
    expiration_timestamp >= $7
ORDER BY gas_amount * gas_price * sell_token_price ASC
LIMIT 1
    "#;
    sqlx::query_as(QUERY)
        .bind(&params.sell_token)
        .bind(&params.buy_token)
        .bind(&params.sell_amount_0)
        .bind(&params.sell_amount_1)
        .bind(&params.buy_amount)
        .bind(params.kind)
        .bind(params.expiration)
        .fetch_optional(ex)
        .await
}

pub async fn remove_expired_quotes(
    ex: &mut PgConnection,
    max_expiry: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
DELETE FROM quotes
WHERE expiration_timestamp < $1
    "#;
    sqlx::query(QUERY)
        .bind(max_expiry)
        .execute(ex)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::byte_array::ByteArray;
    use chrono::Duration;
    use sqlx::{types::chrono::TimeZone, Connection};

    /// The postgres database in our CI has different datetime precision than
    /// the `DateTime` uses. This leads to issues comparing round-tripped data.
    /// Work around the issue by created `DateTime`s with lower precision.
    fn low_precision_now() -> DateTime<Utc> {
        Utc.timestamp(Utc::now().timestamp(), 0)
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_get_quote_by_id() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        let mut quote = Quote {
            id: Default::default(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: 3.into(),
            buy_amount: 4.into(),
            gas_amount: 5.,
            gas_price: 6.,
            sell_token_price: 7.,
            order_kind: OrderKind::Sell,
            expiration_timestamp: now,
        };
        let id = save(&mut db, &quote).await.unwrap();
        quote.id = id;
        assert_eq!(get(&mut db, id).await.unwrap().unwrap(), quote);

        remove_expired_quotes(&mut db, now + Duration::seconds(30))
            .await
            .unwrap();
        assert_eq!(get(&mut db, id).await.unwrap(), None);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_find_quote() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        let token_a = ByteArray([1; 20]);
        let quote_a = Quote {
            id: Default::default(),
            sell_token: token_a,
            buy_token: ByteArray([3; 20]),
            sell_amount: 4.into(),
            buy_amount: 5.into(),
            order_kind: OrderKind::Sell,
            gas_amount: 1.,
            gas_price: 1.,
            sell_token_price: 1.,
            expiration_timestamp: now,
        };

        let token_b = ByteArray([2; 20]);
        let quote_b = Quote {
            id: Default::default(),
            sell_token: token_b,
            buy_token: token_a,
            sell_amount: 200.into(),
            buy_amount: 100.into(),
            order_kind: OrderKind::Buy,
            gas_amount: 20_000_u32.into(),
            gas_price: 1.,
            sell_token_price: 1.,
            expiration_timestamp: now,
        };

        // Save two measurements for token_a
        let quotes_a = [
            {
                let mut quote = Quote {
                    expiration_timestamp: now,
                    gas_amount: 100_u32.into(),
                    ..quote_a.clone()
                };
                let id = save(&mut db, &quote).await.unwrap();
                quote.id = id;
                quote
            },
            {
                let mut quote = Quote {
                    expiration_timestamp: now + Duration::seconds(60),
                    gas_amount: 200_u32.into(),
                    ..quote_a.clone()
                };
                let id = save(&mut db, &quote).await.unwrap();
                quote.id = id;
                quote
            },
        ];

        // Save one measurement for token_b
        let quotes_b = [{
            let mut quote = Quote {
                expiration_timestamp: now,
                gas_amount: 10_u32.into(),
                ..quote_b.clone()
            };
            let id = save(&mut db, &quote).await.unwrap();
            quote.id = id;
            quote
        }];

        // Token A has readings valid until now and in 30s
        let search_a = QuoteSearchParameters {
            sell_token: quote_a.sell_token,
            buy_token: quote_a.buy_token,
            sell_amount_0: quote_a.sell_amount.clone(),
            sell_amount_1: quote_a.sell_amount.clone(),
            buy_amount: 1.into(),
            kind: quote_a.order_kind,
            expiration: now,
        };
        assert_eq!(
            find(&mut db, &search_a).await.unwrap().unwrap(),
            quotes_a[0],
        );
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    expiration: now + Duration::seconds(30),
                    ..search_a.clone()
                }
            )
            .await
            .unwrap()
            .unwrap(),
            quotes_a[1]
        );

        // Token A has readings for sell + fee amount equal to quoted amount.
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    sell_amount_0: quote_a.sell_amount.clone() - BigDecimal::from(1),
                    sell_amount_1: quote_a.sell_amount.clone(),
                    ..search_a.clone()
                },
            )
            .await
            .unwrap()
            .unwrap(),
            quotes_a[0],
        );

        // Token A has no reading for wrong filter
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    sell_amount_0: quote_a.sell_amount.clone() - BigDecimal::from(1),
                    sell_amount_1: quote_a.sell_amount.clone() - BigDecimal::from(1),
                    ..search_a.clone()
                }
            )
            .await
            .unwrap(),
            None
        );

        // Token B only has readings valid until now
        let search_b = QuoteSearchParameters {
            sell_token: quote_b.sell_token,
            buy_token: quote_b.buy_token,
            sell_amount_0: 999.into(),
            sell_amount_1: 999.into(),
            buy_amount: quote_b.buy_amount,
            kind: quote_b.order_kind,
            expiration: now,
        };
        assert_eq!(
            find(&mut db, &search_b).await.unwrap().unwrap(),
            quotes_b[0],
        );
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    expiration: now + Duration::seconds(30),
                    ..search_b.clone()
                }
            )
            .await
            .unwrap(),
            None
        );

        // Token B has no reading for wrong filter
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    buy_amount: 99.into(),
                    ..search_b.clone()
                }
            )
            .await
            .unwrap(),
            None
        );

        // Query that previously succeeded after cleaning up expired measurements.
        remove_expired_quotes(&mut db, now + Duration::seconds(120))
            .await
            .unwrap();
        assert_eq!(find(&mut db, &search_a).await.unwrap(), None);
        assert_eq!(find(&mut db, &search_b).await.unwrap(), None);
    }
}

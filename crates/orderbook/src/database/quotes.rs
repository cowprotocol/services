use super::{orders::DbOrderKind, Postgres};
use crate::{
    conversions::*,
    fee_subsidy::FeeParameters,
    order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
};
use anyhow::{Context, Result};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use model::quote::QuoteId;
use primitive_types::H160;
use shared::maintenance::Maintaining;

#[derive(sqlx::FromRow)]
struct QuoteRow {
    id: QuoteId,
    sell_token: database::Address,
    buy_token: database::Address,
    sell_amount: BigDecimal,
    buy_amount: BigDecimal,
    gas_amount: f64,
    gas_price: f64,
    sell_token_price: f64,
    order_kind: DbOrderKind,
    expiration_timestamp: DateTime<Utc>,
}

impl TryFrom<QuoteRow> for QuoteData {
    type Error = anyhow::Error;

    fn try_from(row: QuoteRow) -> Result<QuoteData> {
        Ok(QuoteData {
            sell_token: H160(row.sell_token.0),
            buy_token: H160(row.buy_token.0),
            quoted_sell_amount: big_decimal_to_u256(&row.sell_amount)
                .context("quoted sell amount is not a valid U256")?,
            quoted_buy_amount: big_decimal_to_u256(&row.buy_amount)
                .context("quoted buy amount is not a valid U256")?,
            fee_parameters: FeeParameters {
                gas_amount: row.gas_amount,
                gas_price: row.gas_price,
                sell_token_price: row.sell_token_price,
            },
            kind: row.order_kind.into(),
            expiration: row.expiration_timestamp,
        })
    }
}

#[async_trait::async_trait]
impl QuoteStoring for Postgres {
    async fn save(&self, data: QuoteData) -> Result<Option<QuoteId>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_quote"])
            .start_timer();

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
        ;"#;

        let (id,) = sqlx::query_as(QUERY)
            .bind(data.sell_token.as_bytes())
            .bind(data.buy_token.as_bytes())
            .bind(u256_to_big_decimal(&data.quoted_sell_amount))
            .bind(u256_to_big_decimal(&data.quoted_buy_amount))
            .bind(&data.fee_parameters.gas_amount)
            .bind(&data.fee_parameters.gas_price)
            .bind(&data.fee_parameters.sell_token_price)
            .bind(DbOrderKind::from(data.kind))
            .bind(data.expiration)
            .fetch_one(&self.pool)
            .await
            .context("failed to insert quote")?;

        Ok(Some(id))
    }

    async fn get(&self, id: QuoteId) -> Result<Option<QuoteData>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_quote"])
            .start_timer();

        const QUERY: &str = r#"
            SELECT *
            FROM quotes
            WHERE id = $1
        ;"#;

        let quote: Option<QuoteRow> = sqlx::query_as(QUERY)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .context("failed to get quote by ID")?;

        quote.map(TryFrom::try_from).transpose()
    }

    async fn find(
        &self,
        parameters: QuoteSearchParameters,
        expiration: DateTime<Utc>,
    ) -> Result<Option<(QuoteId, QuoteData)>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["find_quote"])
            .start_timer();

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
        ;"#;

        let quote: Option<QuoteRow> = sqlx::query_as(QUERY)
            .bind(parameters.sell_token.as_bytes())
            .bind(parameters.buy_token.as_bytes())
            .bind(u256_to_big_decimal(&parameters.sell_amount))
            .bind(u256_to_big_decimal(
                &(parameters.sell_amount + parameters.fee_amount),
            ))
            .bind(u256_to_big_decimal(&parameters.buy_amount))
            .bind(DbOrderKind::from(parameters.kind))
            .bind(expiration)
            .fetch_optional(&self.pool)
            .await
            .context("failed finding quote by parameters")?;

        quote
            .map(|quote| Ok((quote.id, quote.try_into()?)))
            .transpose()
    }
}

impl Postgres {
    pub async fn remove_expired_quotes(&self, max_expiry: DateTime<Utc>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["remove_expired_quotes"])
            .start_timer();

        const QUERY: &str = "DELETE FROM quotes WHERE expiration_timestamp < $1;";
        sqlx::query(QUERY)
            .bind(max_expiry)
            .execute(&self.pool)
            .await
            .context("remove_expired_quotes failed")
            .map(|_| ())
    }
}

#[async_trait::async_trait]
impl Maintaining for Postgres {
    async fn run_maintenance(&self) -> Result<()> {
        self.remove_expired_quotes(Utc::now())
            .await
            .context("fee measurement maintenance error")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone as _};
    use ethcontract::U256;
    use model::order::OrderKind;
    use primitive_types::H160;

    /// The postgres database in our CI has different datetime precision than
    /// the `DateTime` uses. This leads to issues comparing round-tripped data.
    /// Work around the issue by created `DateTime`s with lower precision.
    fn low_precision_now() -> DateTime<Utc> {
        Utc.timestamp(Utc::now().timestamp(), 0)
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_get_quote_by_id() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let now = low_precision_now();
        let quote = QuoteData {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            quoted_sell_amount: 3.into(),
            quoted_buy_amount: 4.into(),
            fee_parameters: 5_u32.into(),
            kind: OrderKind::Sell,
            expiration: now,
        };
        let id = db.save(quote.clone()).await.unwrap().unwrap();

        assert_eq!(db.get(id).await.unwrap().unwrap(), quote);

        db.remove_expired_quotes(now + Duration::seconds(30))
            .await
            .unwrap();

        assert_eq!(db.get(id).await.unwrap(), None);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_find_quote() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let now = low_precision_now();
        let token_a = H160::from_low_u64_be(1);
        let quote_a = QuoteData {
            sell_token: token_a,
            buy_token: H160::from_low_u64_be(3),
            quoted_sell_amount: 4.into(),
            quoted_buy_amount: 5.into(),
            kind: OrderKind::Sell,
            ..Default::default()
        };

        let token_b = H160::from_low_u64_be(2);
        let quote_b = QuoteData {
            sell_token: token_b,
            buy_token: token_a,
            quoted_sell_amount: 200.into(),
            quoted_buy_amount: 100.into(),
            fee_parameters: 20_000_u32.into(),
            kind: OrderKind::Buy,
            expiration: now,
        };

        // Save two measurements for token_a
        let quotes_a = [
            {
                let quote = QuoteData {
                    expiration: now,
                    fee_parameters: 100_u32.into(),
                    ..quote_a.clone()
                };
                let id = db.save(quote.clone()).await.unwrap().unwrap();

                (id, quote)
            },
            {
                let quote = QuoteData {
                    expiration: now + Duration::seconds(60),
                    fee_parameters: 200_u32.into(),
                    ..quote_a.clone()
                };
                let id = db.save(quote.clone()).await.unwrap().unwrap();

                (id, quote)
            },
        ];

        // Save one measurement for token_b
        let quotes_b = [{
            let quote = QuoteData {
                expiration: now,
                fee_parameters: 10_u32.into(),
                ..quote_b.clone()
            };
            let id = db.save(quote.clone()).await.unwrap().unwrap();

            (id, quote)
        }];

        // Token A has readings valid until now and in 30s
        let search_a = QuoteSearchParameters {
            sell_token: quote_a.sell_token,
            buy_token: quote_a.buy_token,
            sell_amount: quote_a.quoted_sell_amount,
            buy_amount: 1.into(),
            fee_amount: 0.into(),
            kind: quote_a.kind,
            ..Default::default()
        };
        assert_eq!(
            db.find(search_a.clone(), now).await.unwrap().unwrap(),
            quotes_a[0],
        );
        assert_eq!(
            db.find(search_a.clone(), now + Duration::seconds(30))
                .await
                .unwrap()
                .unwrap(),
            quotes_a[1],
        );

        // Token A has readings for sell + fee amount equal to quoted amount.
        assert_eq!(
            db.find(
                QuoteSearchParameters {
                    sell_amount: quote_a.quoted_sell_amount - U256::from(1),
                    fee_amount: 1.into(),
                    ..search_a.clone()
                },
                now
            )
            .await
            .unwrap()
            .unwrap(),
            quotes_a[0],
        );
        assert_eq!(
            db.find(search_a.clone(), now + Duration::seconds(30))
                .await
                .unwrap()
                .unwrap(),
            quotes_a[1],
        );

        // Token A has no reading for wrong filter
        assert_eq!(
            db.find(
                QuoteSearchParameters {
                    sell_amount: quote_a.quoted_sell_amount - U256::from(1),
                    ..search_a.clone()
                },
                now
            )
            .await
            .unwrap(),
            None
        );

        // Token B only has readings valid until now
        let search_b = QuoteSearchParameters {
            sell_token: quote_b.sell_token,
            buy_token: quote_b.buy_token,
            sell_amount: 999.into(),
            buy_amount: quote_b.quoted_buy_amount,
            fee_amount: 0.into(),
            kind: quote_b.kind,
            ..Default::default()
        };
        assert_eq!(
            db.find(search_b.clone(), now).await.unwrap().unwrap(),
            quotes_b[0],
        );
        assert_eq!(
            db.find(search_b.clone(), now + Duration::seconds(30))
                .await
                .unwrap(),
            None
        );

        // Token B has no reading for wrong filter
        assert_eq!(
            db.find(
                QuoteSearchParameters {
                    buy_amount: 99.into(),
                    ..search_b.clone()
                },
                now
            )
            .await
            .unwrap(),
            None
        );

        // Query that previously succeeded after cleaning up expired measurements.
        db.remove_expired_quotes(now + Duration::seconds(120))
            .await
            .unwrap();
        assert_eq!(db.find(search_a, now).await.unwrap(), None);
        assert_eq!(db.find(search_b, now).await.unwrap(), None);
    }
}

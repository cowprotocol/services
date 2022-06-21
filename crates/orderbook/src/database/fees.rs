use super::{orders::DbOrderKind, Postgres};
use crate::{
    conversions::*,
    fee::{FeeData, MinFeeStoring},
    fee_subsidy::FeeParameters,
    order_quoting::{QuoteData, QuoteSearchParameters, QuoteStoring},
};
use anyhow::{Context, Result};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use model::{order::OrderKind, quote::QuoteId};
use shared::maintenance::Maintaining;

#[derive(sqlx::FromRow)]
struct QuoteRow {
    id: QuoteId,
    sell_token: Vec<u8>,
    buy_token: Vec<u8>,
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
            sell_token: h160_from_vec(row.sell_token)?,
            buy_token: h160_from_vec(row.buy_token)?,
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

    async fn get(&self, id: QuoteId, _: DateTime<Utc>) -> Result<Option<QuoteData>> {
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
        const QUERY: &str = r#"
            SELECT *
            FROM quotes
            WHERE
                sell_token = $1 AND
                buy_token = $2 AND
                (
                    order_kind = 'sell' AND sell_amount = $3 OR
                    order_kind = 'sell' AND sell_amount = $4 OR
                    order_kind = 'buy' AND buy_amount = $5
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

#[derive(sqlx::FromRow)]
struct FeeRow {
    gas_amount: f64,
    gas_price: f64,
    sell_token_price: f64,
}

impl FeeRow {
    fn into_fee(self) -> FeeParameters {
        FeeParameters {
            gas_amount: self.gas_amount,
            gas_price: self.gas_price,
            sell_token_price: self.sell_token_price,
        }
    }
}

#[async_trait::async_trait]
impl MinFeeStoring for Postgres {
    async fn save_fee_measurement(
        &self,
        fee_data: FeeData,
        expiry: DateTime<Utc>,
        estimate: FeeParameters,
    ) -> Result<()> {
        let (sell_amount, buy_amount) = match fee_data.kind {
            OrderKind::Buy => (0.into(), fee_data.amount),
            OrderKind::Sell => (fee_data.amount, 0.into()),
        };
        let quote = QuoteData {
            sell_token: fee_data.sell_token,
            buy_token: fee_data.buy_token,
            quoted_sell_amount: sell_amount,
            quoted_buy_amount: buy_amount,
            fee_parameters: estimate,
            kind: fee_data.kind,
            expiration: expiry,
        };

        self.save(quote).await?;
        Ok(())
    }

    async fn find_measurement_exact(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<FeeParameters>> {
        let (sell_amount, buy_amount) = match fee_data.kind {
            OrderKind::Buy => (0.into(), fee_data.amount),
            OrderKind::Sell => (fee_data.amount, 0.into()),
        };
        let search = QuoteSearchParameters {
            sell_token: fee_data.sell_token,
            buy_token: fee_data.buy_token,
            sell_amount,
            buy_amount,
            kind: fee_data.kind,
            ..Default::default()
        };

        let quote = self.find(search, min_expiry).await?;
        Ok(quote.map(|(_, quote)| quote.fee_parameters))
    }

    async fn find_measurement_including_larger_amount(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<FeeParameters>> {
        // Same as above but with `amount >=` instead of `=`.
        const QUERY: &str = "\
            SELECT gas_amount, gas_price, sell_token_price FROM quotes \
            WHERE
                sell_token = $1 AND \
                buy_token = $2 AND \
                sell_amount >= $3 AND \
                buy_amount = $4 AND \
                order_kind = $5 AND \
                expiration_timestamp >= $6 \
            ORDER BY gas_amount * gas_price * sell_token_price ASC \
            LIMIT 1 \
            ;";

        let (sell_amount, buy_amount) = match fee_data.kind {
            OrderKind::Buy => (0.into(), fee_data.amount),
            OrderKind::Sell => (fee_data.amount, 0.into()),
        };
        let result: Option<FeeRow> = sqlx::query_as(QUERY)
            .bind(fee_data.sell_token.as_bytes())
            .bind(fee_data.buy_token.as_bytes())
            .bind(u256_to_big_decimal(&sell_amount))
            .bind(u256_to_big_decimal(&buy_amount))
            .bind(DbOrderKind::from(fee_data.kind))
            .bind(min_expiry)
            .fetch_optional(&self.pool)
            .await
            .context("find_measurement_including_larger_amount")?;
        Ok(result.map(FeeRow::into_fee))
    }
}

impl Postgres {
    pub async fn remove_expired_quotes(&self, max_expiry: DateTime<Utc>) -> Result<()> {
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
    use chrono::Duration;
    use ethcontract::U256;
    use model::order::OrderKind;
    use primitive_types::H160;

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_get_quote_by_id() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let now = Utc::now();
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

        assert_eq!(db.get(id, now).await.unwrap().unwrap(), quote);
        assert_eq!(
            db.get(id, now + Duration::seconds(30))
                .await
                .unwrap()
                .unwrap(),
            quote
        );

        db.remove_expired_quotes(now + Duration::seconds(30))
            .await
            .unwrap();
        assert_eq!(db.get(id, now).await.unwrap(), None);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_find_quote() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let now = Utc::now();
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

    #[tokio::test]
    #[ignore]
    async fn postgres_find_measurement_including_larger_amount_() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let now = Utc::now();
        let fee_data_a = FeeData {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(3),
            amount: 10.into(),
            kind: OrderKind::Sell,
        };

        db.save_fee_measurement(fee_data_a, now, 100.into())
            .await
            .unwrap();
        db.save_fee_measurement(
            FeeData {
                amount: 20.into(),
                ..fee_data_a
            },
            now,
            200.into(),
        )
        .await
        .unwrap();

        assert_eq!(
            db.find_measurement_including_larger_amount(
                FeeData {
                    amount: 1.into(),
                    ..fee_data_a
                },
                now
            )
            .await
            .unwrap()
            .unwrap(),
            100_u32.into()
        );
        assert_eq!(
            db.find_measurement_including_larger_amount(
                FeeData {
                    amount: 10.into(),
                    ..fee_data_a
                },
                now
            )
            .await
            .unwrap()
            .unwrap(),
            100_u32.into()
        );
        assert_eq!(
            db.find_measurement_including_larger_amount(
                FeeData {
                    amount: 11.into(),
                    ..fee_data_a
                },
                now
            )
            .await
            .unwrap()
            .unwrap(),
            200_u32.into()
        );
        assert_eq!(
            db.find_measurement_including_larger_amount(
                FeeData {
                    amount: 20.into(),
                    ..fee_data_a
                },
                now
            )
            .await
            .unwrap()
            .unwrap(),
            200_u32.into()
        );
        assert_eq!(
            db.find_measurement_including_larger_amount(
                FeeData {
                    amount: 21.into(),
                    ..fee_data_a
                },
                now
            )
            .await
            .unwrap(),
            None
        );
    }
}

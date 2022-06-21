use super::{orders::DbOrderKind, Postgres};
use crate::{
    conversions::*,
    fee::{FeeData, MinFeeStoring},
    fee_subsidy::FeeParameters,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use model::order::OrderKind;
use shared::maintenance::Maintaining;

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
        const QUERY: &str =
            "INSERT INTO quotes (sell_token, buy_token, sell_amount, buy_amount, order_kind, expiration_timestamp, gas_amount, gas_price, sell_token_price) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9);";
        let (sell_amount, buy_amount) = match fee_data.kind {
            OrderKind::Buy => (0.into(), fee_data.amount),
            OrderKind::Sell => (fee_data.amount, 0.into()),
        };
        sqlx::query(QUERY)
            .bind(fee_data.sell_token.as_bytes())
            .bind(fee_data.buy_token.as_bytes())
            .bind(u256_to_big_decimal(&sell_amount))
            .bind(u256_to_big_decimal(&buy_amount))
            .bind(DbOrderKind::from(fee_data.kind))
            .bind(expiry)
            .bind(&estimate.gas_amount)
            .bind(&estimate.gas_price)
            .bind(&estimate.sell_token_price)
            .execute(&self.pool)
            .await
            .context("insert MinFeeMeasurement failed")
            .map(|_| ())
    }

    async fn find_measurement_exact(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<FeeParameters>> {
        // Fetches the lowest fee estimate we may have for the user
        const QUERY: &str = "\
            SELECT gas_amount, gas_price, sell_token_price FROM quotes \
            WHERE
                sell_token = $1 AND \
                buy_token = $2 AND \
                sell_amount = $3 AND \
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
            .context("find_measurement_exact")?;
        Ok(result.map(FeeRow::into_fee))
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
            .context("remove_expired_fee_measurements failed")
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
    use model::order::OrderKind;
    use primitive_types::H160;

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_load_fee_measurements() {
        let db = Postgres::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let now = Utc::now();
        let token_a = H160::from_low_u64_be(1);
        let fee_data_a = FeeData {
            sell_token: token_a,
            buy_token: H160::from_low_u64_be(3),
            amount: 4.into(),
            kind: OrderKind::Sell,
        };
        let token_b = H160::from_low_u64_be(2);
        let fee_data_b = FeeData {
            sell_token: token_b,
            buy_token: token_a,
            amount: 100.into(),
            kind: OrderKind::Buy,
        };

        // Save two measurements for token_a
        db.save_fee_measurement(fee_data_a, now, 100u32.into())
            .await
            .unwrap();
        db.save_fee_measurement(fee_data_a, now + Duration::seconds(60), 200u32.into())
            .await
            .unwrap();

        // Save one measurement for token_b
        db.save_fee_measurement(fee_data_b, now, 10u32.into())
            .await
            .unwrap();

        // Token A has readings valid until now and in 30s
        assert_eq!(
            db.find_measurement_exact(fee_data_a, now)
                .await
                .unwrap()
                .unwrap(),
            100_u32.into()
        );
        assert_eq!(
            db.find_measurement_exact(fee_data_a, now + Duration::seconds(30))
                .await
                .unwrap()
                .unwrap(),
            200u32.into()
        );

        // Token B only has readings valid until now
        assert_eq!(
            db.find_measurement_exact(fee_data_b, now)
                .await
                .unwrap()
                .unwrap(),
            10u32.into()
        );
        assert_eq!(
            db.find_measurement_exact(fee_data_b, now + Duration::seconds(30))
                .await
                .unwrap(),
            None
        );

        // Token B has no reading for wrong filter
        assert_eq!(
            db.find_measurement_exact(
                FeeData {
                    amount: 99.into(),
                    ..fee_data_b
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
        assert_eq!(
            db.find_measurement_exact(fee_data_b, now).await.unwrap(),
            None
        );
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

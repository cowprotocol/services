use super::{orders::DbOrderKind, Postgres};
use crate::{
    conversions::*,
    fee::{FeeData, MinFeeStoring},
};

use anyhow::{anyhow, Context, Result};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use ethcontract::U256;
use shared::maintenance::Maintaining;

#[async_trait::async_trait]
impl MinFeeStoring for Postgres {
    async fn save_fee_measurement(
        &self,
        fee_data: FeeData,
        expiry: DateTime<Utc>,
        min_fee: U256,
    ) -> Result<()> {
        const QUERY: &str =
            "INSERT INTO min_fee_measurements (sell_token, buy_token, amount, order_kind, expiration_timestamp, min_fee) VALUES ($1, $2, $3, $4, $5, $6);";
        sqlx::query(QUERY)
            .bind(fee_data.sell_token.as_bytes())
            .bind(fee_data.buy_token.as_bytes())
            .bind(u256_to_big_decimal(&fee_data.amount))
            .bind(DbOrderKind::from(fee_data.kind))
            .bind(expiry)
            .bind(u256_to_big_decimal(&min_fee))
            .execute(&self.pool)
            .await
            .context("insert MinFeeMeasurement failed")
            .map(|_| ())
    }

    async fn find_measurement_exact(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>> {
        const QUERY: &str = "\
            SELECT MIN(min_fee) FROM min_fee_measurements \
            WHERE
                sell_token = $1 AND \
                buy_token = $2 AND \
                amount = $3 AND \
                order_kind = $4 AND \
                expiration_timestamp >= $5 \
            ;";

        let result: Option<BigDecimal> = sqlx::query_scalar(QUERY)
            .bind(fee_data.sell_token.as_bytes())
            .bind(fee_data.buy_token.as_bytes())
            .bind(u256_to_big_decimal(&fee_data.amount))
            .bind(DbOrderKind::from(fee_data.kind))
            .bind(min_expiry)
            .fetch_one(&self.pool)
            .await
            .context("find_measurement_exact")?;
        match result {
            Some(row) => {
                Ok(Some(big_decimal_to_u256(&row).ok_or_else(|| {
                    anyhow!("min fee is not an unsigned integer")
                })?))
            }
            None => Ok(None),
        }
    }

    async fn find_measurement_including_larger_amount(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>> {
        // Same as above but with `amount >=` instead of `=`.
        const QUERY: &str = "\
            SELECT MIN(min_fee) FROM min_fee_measurements \
            WHERE
                sell_token = $1 AND \
                buy_token = $2 AND \
                amount >= $3 AND \
                order_kind = $4 AND \
                expiration_timestamp >= $5 \
            ;";

        let result: Option<BigDecimal> = sqlx::query_scalar(QUERY)
            .bind(fee_data.sell_token.as_bytes())
            .bind(fee_data.buy_token.as_bytes())
            .bind(u256_to_big_decimal(&fee_data.amount))
            .bind(DbOrderKind::from(fee_data.kind))
            .bind(min_expiry)
            .fetch_one(&self.pool)
            .await
            .context("find_measurement_including_larger_amount")?;
        match result {
            Some(row) => {
                Ok(Some(big_decimal_to_u256(&row).ok_or_else(|| {
                    anyhow!("min fee is not an unsigned integer")
                })?))
            }
            None => Ok(None),
        }
    }
}

impl Postgres {
    pub async fn remove_expired_fee_measurements(&self, max_expiry: DateTime<Utc>) -> Result<()> {
        const QUERY: &str = "DELETE FROM min_fee_measurements WHERE expiration_timestamp < $1;";
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
        self.remove_expired_fee_measurements(Utc::now())
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
        db.remove_expired_fee_measurements(now + Duration::seconds(120))
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

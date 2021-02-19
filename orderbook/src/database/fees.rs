use super::Database;
use crate::integer_conversions::*;

use anyhow::{anyhow, Context, Result};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use ethcontract::{H160, U256};

impl Database {
    pub async fn save_fee_measurement(
        &self,
        token: H160,
        expiry: DateTime<Utc>,
        min_fee: U256,
    ) -> Result<()> {
        const QUERY: &str =
            "INSERT INTO min_fee_measurements (token, expiration_timestamp, min_fee) VALUES ($1, $2, $3);";
        sqlx::query(QUERY)
            .bind(token.as_bytes())
            .bind(expiry)
            .bind(u256_to_big_decimal(&min_fee))
            .execute(&self.pool)
            .await
            .context("insert MinFeeMeasurement failed")
            .map(|_| ())
    }

    pub async fn get_min_fee(
        &self,
        token: H160,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>> {
        const QUERY: &str = "\
            SELECT MIN(min_fee) FROM min_fee_measurements \
            WHERE token = $1 AND expiration_timestamp >= $2
            ";

        let result: Option<BigDecimal> = sqlx::query_scalar(QUERY)
            .bind(token.as_bytes())
            .bind(min_expiry)
            .fetch_one(&self.pool)
            .await
            .context("load minimum fee measurement failed")?;
        match result {
            Some(row) => {
                Ok(Some(big_decimal_to_u256(&row).ok_or_else(|| {
                    anyhow!("min fee is not an unsigned integer")
                })?))
            }
            None => Ok(None),
        }
    }

    pub async fn remove_expired(&self, max_expiry: DateTime<Utc>) -> Result<()> {
        const QUERY: &str = "DELETE FROM min_fee_measurements WHERE expiration_timestamp < $1;";
        sqlx::query(QUERY)
            .bind(max_expiry)
            .execute(&self.pool)
            .await
            .context("insert MinFeeMeasurement failed")
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn save_and_load_fee_measurements() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let now = Utc::now();
        let token_a = H160::from_low_u64_be(1);
        let token_b = H160::from_low_u64_be(2);

        db.save_fee_measurement(token_a, now, 100u32.into())
            .await
            .unwrap();
        db.save_fee_measurement(token_a, now + Duration::seconds(60), 200u32.into())
            .await
            .unwrap();
        db.save_fee_measurement(token_b, now, 10u32.into())
            .await
            .unwrap();

        assert_eq!(
            db.get_min_fee(token_a, now).await.unwrap().unwrap(),
            100_u32.into()
        );
        assert_eq!(
            db.get_min_fee(token_a, now + Duration::seconds(30))
                .await
                .unwrap()
                .unwrap(),
            200u32.into()
        );

        assert_eq!(
            db.get_min_fee(token_b, now).await.unwrap().unwrap(),
            10u32.into()
        );
        assert_eq!(
            db.get_min_fee(token_b, now + Duration::seconds(30))
                .await
                .unwrap(),
            None
        );

        db.remove_expired(now + Duration::seconds(120))
            .await
            .unwrap();
        assert_eq!(db.get_min_fee(token_b, now).await.unwrap(), None);
    }
}

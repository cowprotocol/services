use super::{orders::DbOrderKind, Database};
use crate::conversions::*;
use crate::fee::MinFeeStoring;

use anyhow::{anyhow, Context, Result};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use ethcontract::{H160, U256};
use model::order::OrderKind;

#[async_trait::async_trait]
impl MinFeeStoring for Database {
    async fn save_fee_measurement(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
        expiry: DateTime<Utc>,
        min_fee: U256,
    ) -> Result<()> {
        const QUERY: &str =
            "INSERT INTO min_fee_measurements (sell_token, buy_token, amount, order_kind, expiration_timestamp, min_fee) VALUES ($1, $2, $3, $4, $5, $6);";
        sqlx::query(QUERY)
            .bind(sell_token.as_bytes())
            .bind(buy_token.as_bytes())
            .bind(u256_to_big_decimal(&amount))
            .bind(DbOrderKind::from(kind))
            .bind(expiry)
            .bind(u256_to_big_decimal(&min_fee))
            .execute(&self.pool)
            .await
            .context("insert MinFeeMeasurement failed")
            .map(|_| ())
    }

    async fn get_min_fee(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>> {
        const QUERY: &str = "\
            SELECT MIN(min_fee) FROM min_fee_measurements \
            WHERE sell_token = $1 \
            AND buy_token = $2 \
            AND amount = $3 \
            AND order_kind = $4 \
            AND expiration_timestamp >= $5
            ";

        let result: Option<BigDecimal> = sqlx::query_scalar(QUERY)
            .bind(sell_token.as_bytes())
            .bind(buy_token.as_bytes())
            .bind(u256_to_big_decimal(&amount))
            .bind(DbOrderKind::from(kind))
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
}

impl Database {
    pub async fn remove_expired_fee_measurements(&self, max_expiry: DateTime<Utc>) -> Result<()> {
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

        // Save two measurements for token_a
        db.save_fee_measurement(
            token_a,
            token_b,
            100.into(),
            OrderKind::Sell,
            now,
            100u32.into(),
        )
        .await
        .unwrap();
        db.save_fee_measurement(
            token_a,
            token_b,
            100.into(),
            OrderKind::Sell,
            now + Duration::seconds(60),
            200u32.into(),
        )
        .await
        .unwrap();

        // Save one measurement for token_b
        db.save_fee_measurement(
            token_b,
            token_a,
            100.into(),
            OrderKind::Buy,
            now,
            10u32.into(),
        )
        .await
        .unwrap();

        // Token A has readings valid until now and in 30s
        assert_eq!(
            db.get_min_fee(token_a, token_b, 100.into(), OrderKind::Sell, now)
                .await
                .unwrap()
                .unwrap(),
            100_u32.into()
        );
        assert_eq!(
            db.get_min_fee(
                token_a,
                token_b,
                100.into(),
                OrderKind::Sell,
                now + Duration::seconds(30)
            )
            .await
            .unwrap()
            .unwrap(),
            200u32.into()
        );

        // Token B only has readings valid until now
        assert_eq!(
            db.get_min_fee(token_b, token_a, 100.into(), OrderKind::Buy, now)
                .await
                .unwrap()
                .unwrap(),
            10u32.into()
        );
        assert_eq!(
            db.get_min_fee(
                token_b,
                token_a,
                100.into(),
                OrderKind::Buy,
                now + Duration::seconds(30)
            )
            .await
            .unwrap(),
            None
        );

        db.remove_expired_fee_measurements(now + Duration::seconds(120))
            .await
            .unwrap();
        assert_eq!(
            db.get_min_fee(token_b, token_a, 100.into(), OrderKind::Buy, now)
                .await
                .unwrap(),
            None
        );
    }
}

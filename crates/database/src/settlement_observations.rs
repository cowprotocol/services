use {crate::PgTransaction, bigdecimal::BigDecimal};

#[derive(Debug, Clone, Default, PartialEq, sqlx::FromRow)]
pub struct Observation {
    pub gas_used: BigDecimal,
    pub effective_gas_price: BigDecimal,
    pub surplus: BigDecimal,
    pub fee: BigDecimal,
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn insert(
    ex: &mut PgTransaction<'_>,
    observation: Observation,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO settlement_observations (gas_used, effective_gas_price, surplus, fee, block_number, log_index)
VALUES ($1, $2, $3, $4, $5, $6)
    ;"#;
    sqlx::query(QUERY)
        .bind(observation.gas_used)
        .bind(observation.effective_gas_price)
        .bind(observation.surplus)
        .bind(observation.fee)
        .bind(observation.block_number)
        .bind(observation.log_index)
        .execute(ex)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::events::EventIndex,
        sqlx::{Connection, PgConnection},
    };

    // helper function to make roundtrip possible
    pub async fn fetch(
        ex: &mut PgConnection,
        event: &EventIndex,
    ) -> Result<Option<Observation>, sqlx::Error> {
        const QUERY: &str =
            r#"SELECT * FROM settlement_observations WHERE block_number = $1 AND log_index = $2"#;
        sqlx::query_as(QUERY)
            .bind(event.block_number)
            .bind(event.log_index)
            .fetch_optional(ex)
            .await
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let input = Observation {
            gas_used: 1.into(),
            effective_gas_price: 2.into(),
            surplus: 3.into(),
            fee: 4.into(),
            block_number: 1,
            log_index: 1,
        };

        insert(&mut db, input.clone()).await.unwrap();
        let output = fetch(
            &mut db,
            &EventIndex {
                block_number: 1,
                log_index: 1,
            },
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(input, output);
    }
}

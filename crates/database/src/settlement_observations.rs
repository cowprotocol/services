use {crate::events::EventIndex, bigdecimal::BigDecimal, sqlx::PgConnection};

pub async fn get_settlement_event_without_observation(
    ex: &mut PgConnection,
    max_block_number: i64,
) -> Result<Option<EventIndex>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT block_number, log_index
FROM settlement_observations
WHERE gas_used IS NULL AND block_number <= $1
LIMIT 1
    "#;
    sqlx::query_as(QUERY)
        .bind(max_block_number)
        .fetch_optional(ex)
        .await
}

#[derive(Debug, Clone, Default, PartialEq, sqlx::FromRow)]
pub struct Observation {
    pub gas_used: BigDecimal,
    pub effective_gas_price: BigDecimal,
    pub surplus: BigDecimal,
    pub fee: BigDecimal,
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn update(ex: &mut PgConnection, observation: Observation) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlement_observations
SET gas_used = $1, effective_gas_price = $2, surplus = $3, fee = $4
WHERE block_number = $5 AND log_index = $6
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
    use {super::*, crate::events::EventIndex, sqlx::Connection};

    // helper function to make roundtrip possible
    pub async fn insert(ex: &mut PgConnection, event: &EventIndex) -> Result<(), sqlx::Error> {
        const QUERY: &str = r#"
        INSERT INTO settlement_observations (block_number, log_index) 
        VALUES ($1, $2) ON CONFLICT DO NOTHING;"#;
        sqlx::query(QUERY)
            .bind(event.block_number)
            .bind(event.log_index)
            .execute(ex)
            .await?;
        Ok(())
    }

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

        // insert event without observation
        let event = EventIndex {
            block_number: 1,
            log_index: 1,
        };
        insert(&mut db, &event).await.unwrap();

        // there is one settlement event without observation
        let result = get_settlement_event_without_observation(&mut db, 2)
            .await
            .unwrap();
        assert_eq!(result, Some(event));

        // update existing row
        let input = Observation {
            gas_used: 5.into(),
            effective_gas_price: 6.into(),
            surplus: 7.into(),
            fee: 8.into(),
            block_number: 1,
            log_index: 1,
        };
        update(&mut db, input.clone()).await.unwrap();
        let output = fetch(&mut db, &event).await.unwrap().unwrap();
        assert_eq!(input, output);

        // since updated, no more events without observations
        let result = get_settlement_event_without_observation(&mut db, 2)
            .await
            .unwrap();
        assert_eq!(result, None,);
    }
}

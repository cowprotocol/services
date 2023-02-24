use {bigdecimal::BigDecimal, sqlx::PgConnection};

#[derive(Debug, PartialEq, sqlx::FromRow)]
pub struct SettlementEvent {
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn get_settlement_event_without_observation(
    ex: &mut PgConnection,
    max_block_number: i64,
) -> Result<Option<SettlementEvent>, sqlx::Error> {
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

#[derive(Debug, Default, PartialEq, sqlx::FromRow)]
pub struct Observation {
    pub gas_used: Option<BigDecimal>,
    pub effective_gas_price: Option<BigDecimal>,
    pub surplus: Option<BigDecimal>,
    pub fee: Option<BigDecimal>,
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn update(
    ex: &mut PgConnection,
    block_number: i64,
    log_index: i64,
    gas_used: BigDecimal,
    effective_gas_price: BigDecimal,
    surplus: BigDecimal,
    fee: BigDecimal,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlement_observations
SET gas_used = $1, effective_gas_price = $2, surplus = $3, fee = $4
WHERE block_number = $5 AND log_index = $6
    ;"#;
    sqlx::query(QUERY)
        .bind(gas_used)
        .bind(effective_gas_price)
        .bind(surplus)
        .bind(fee)
        .bind(block_number)
        .bind(log_index)
        .execute(ex)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, crate::events::EventIndex, sqlx::Connection};

    // helper function to make roundtrip possible
    pub async fn insert(
        ex: &mut PgConnection,
        block_number: i64,
        log_index: i64,
    ) -> Result<(), sqlx::Error> {
        const QUERY: &str = r#"
        INSERT INTO settlement_observations (block_number, log_index) 
        VALUES ($1, $2) ON CONFLICT DO NOTHING;"#;
        sqlx::query(QUERY)
            .bind(block_number)
            .bind(log_index)
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

        let event = EventIndex {
            block_number: 1,
            log_index: 1,
        };
        insert(&mut db, event.block_number, event.log_index)
            .await
            .unwrap();

        let result = fetch(&mut db, &event).await.unwrap();
        assert_eq!(
            result,
            Some(Observation {
                block_number: 1,
                log_index: 1,
                ..Default::default()
            })
        );

        // there is one settlement event without observation
        let result = get_settlement_event_without_observation(&mut db, 2)
            .await
            .unwrap();
        assert_eq!(
            result,
            Some(SettlementEvent {
                block_number: 1,
                log_index: 1,
            })
        );

        // update existing row
        update(
            &mut db,
            event.block_number,
            event.log_index,
            5.into(),
            6.into(),
            7.into(),
            8.into(),
        )
        .await
        .unwrap();
        let result = fetch(&mut db, &event).await.unwrap();
        assert_eq!(
            result,
            Some(Observation {
                gas_used: Some(5.into()),
                effective_gas_price: Some(6.into()),
                surplus: Some(7.into()),
                fee: Some(8.into()),
                block_number: 1,
                log_index: 1,
            })
        );

        // since updated, no more events without observations
        let result = get_settlement_event_without_observation(&mut db, 2)
            .await
            .unwrap();
        assert_eq!(result, None,);
    }
}

use {crate::events::EventIndex, bigdecimal::BigDecimal, sqlx::PgConnection};

#[derive(Debug, PartialEq, sqlx::FromRow)]
pub struct Observation {
    pub gas_used: BigDecimal,
    pub effective_gas_price: BigDecimal,
    pub surplus: BigDecimal,
    pub fee: BigDecimal,
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn upsert(
    ex: &mut PgConnection,
    event: &EventIndex,
    gas_used: BigDecimal,
    effective_gas_price: BigDecimal,
    surplus: BigDecimal,
    fee: BigDecimal,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
    INSERT INTO settlement_observations (gas_used, effective_gas_price, surplus, fee, block_number, log_index) 
    VALUES ($1, $2, $3, $4, $5, $6)
    ON CONFLICT (block_number, log_index) DO UPDATE SET gas_used = $1, effective_gas_price = $2, surplus = $3, fee = $4;
    "#;
    sqlx::query(QUERY)
        .bind(gas_used)
        .bind(effective_gas_price)
        .bind(surplus)
        .bind(fee)
        .bind(event.block_number)
        .bind(event.log_index)
        .execute(ex)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

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
        upsert(&mut db, &event, 1.into(), 2.into(), 3.into(), 4.into())
            .await
            .unwrap();

        let result = fetch(&mut db, &event).await.unwrap();
        assert_eq!(
            result,
            Some(Observation {
                gas_used: 1.into(),
                effective_gas_price: 2.into(),
                surplus: 3.into(),
                fee: 4.into(),
                block_number: 1,
                log_index: 1,
            })
        );
    }
}

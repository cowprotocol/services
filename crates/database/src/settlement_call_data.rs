use {crate::auction::AuctionId, sqlx::PgConnection};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SettlementCallData {
    pub auction_id: AuctionId,
    pub call_data: Vec<u8>,
    pub uninternalized_call_data: Vec<u8>,
}

pub async fn insert(ex: &mut PgConnection, row: SettlementCallData) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"INSERT INTO settlement_call_data (auction_id, call_data, uninternalized_call_data) VALUES ($1, $2, $3);"#;
    sqlx::query(QUERY)
        .bind(row.auction_id)
        .bind(row.call_data.as_slice())
        .bind(row.uninternalized_call_data.as_slice())
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Option<SettlementCallData>, sqlx::Error> {
    const QUERY: &str = r#"SELECT * FROM settlement_call_data WHERE auction_id = $1"#;
    sqlx::query_as(QUERY)
        .bind(auction_id)
        .fetch_optional(ex)
        .await
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let input = SettlementCallData {
            auction_id: 1,
            call_data: vec![2; 20],
            uninternalized_call_data: vec![3; 20],
        };
        insert(&mut db, input.clone()).await.unwrap();

        let output = fetch(&mut db, 1).await.unwrap().unwrap();
        assert_eq!(input, output);
    }
}

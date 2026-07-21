//! This file contains all functions related to reading or updating
//! data about the competition during an auction in the old JSON based
//! table.
//! See `solver_competition_v2.rs` for the new version of this which
//! uses individual and well defined tables for this.

use {crate::auction::AuctionId, sqlx::PgConnection, tracing::instrument};

#[instrument(skip_all)]
pub async fn save(
    ex: &mut PgConnection,
    id: AuctionId,
    json_string: &str,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO solver_competitions (id, json)
VALUES ($1, $2::jsonb)
    ;"#;
    sqlx::query(QUERY)
        .bind(id)
        .bind(json_string)
        .execute(ex)
        .await?;
    Ok(())
}

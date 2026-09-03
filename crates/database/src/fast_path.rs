//! Database queries for the fast-path settlement feature.
//!
//! Fast-path orders reuse a quote's synthetic solver competition as the
//! actual settlement. This module owns the promotion step that patches
//! the placeholder rows written at quote time to reference the real
//! `order_uid` ([`finalize_quote_competition`]).

use {
    crate::{OrderUid, PgTransaction, auction::AuctionId},
    std::ops::DerefMut,
    tracing::instrument,
};

/// Because the final order uid is not known when we store the quote
/// competition data we use `0x000...000` as a sentinel value.
/// When an order gets placed referencing a quote competition this function
/// replaces the placeholder value with the now final order uid.
#[instrument(skip_all)]
pub async fn finalize_quote_competition(
    ex: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    order_uid: OrderUid,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
WITH patch_te AS (
    UPDATE proposed_trade_executions
    SET order_uid = $1
    WHERE auction_id = $2 AND order_uid = $3
)
UPDATE competition_auctions
SET order_uids = ARRAY[$1]
WHERE id = $2
"#;
    sqlx::query(QUERY)
        .bind(order_uid)
        .bind(auction_id)
        .bind(OrderUid::default())
        .execute(ex.deref_mut())
        .await?;
    Ok(())
}

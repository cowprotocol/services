use {
    crate::{boundary, domain},
    database::order_penalty_caps::OrderPenaltyCap,
    eth_domain_types as eth,
    number::conversions::u256_to_big_decimal,
    sqlx::PgConnection,
};

pub async fn insert_batch(
    ex: &mut PgConnection,
    auction_id: domain::auction::Id,
    penalty_caps: impl IntoIterator<Item = (domain::OrderUid, eth::Ether)>,
) -> Result<(), sqlx::Error> {
    let penalty_caps = penalty_caps
        .into_iter()
        .map(|(order_uid, cap)| OrderPenaltyCap {
            auction_id,
            order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
            penalty_cap_native: u256_to_big_decimal(&cap.0),
        });

    database::order_penalty_caps::insert_batch(ex, penalty_caps).await
}

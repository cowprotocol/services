use {
    crate::{domain, infra::persistence::dto},
    sqlx::PgConnection,
};

pub async fn insert_batch(
    ex: &mut PgConnection,
    auction_id: domain::auction::Id,
    fee_policies: impl IntoIterator<Item = (domain::OrderUid, Vec<domain::fee::Policy>)>,
) -> Result<(), sqlx::Error> {
    let fee_policies = fee_policies.into_iter().flat_map(|(order_uid, policies)| {
        policies
            .into_iter()
            .map(move |policy| dto::fee_policy::from_domain(auction_id, order_uid, policy))
    });

    database::fee_policies::insert_batch(ex, fee_policies).await
}

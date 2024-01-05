use {
    crate::{driver_model::solve::FeePolicy, protocol::fee},
    anyhow::Context,
    database::byte_array::ByteArray,
    model::order::OrderUid,
};

impl super::Persistence {
    pub async fn store_fee_policies(
        &self,
        auction_id: model::auction::AuctionId,
        order_uids: impl Iterator<Item = OrderUid>,
        fee_policies: &fee::Policies,
    ) -> anyhow::Result<()> {
        let mut ex = self.postgres.clone().pool.begin().await.context("begin")?;
        for order_uid in order_uids {
            for fee_policy in fee_policies.get(&order_uid).unwrap_or_default() {
                let fee_policy_dto = database::fee_policies::FeePolicy {
                    auction_id,
                    order_uid: ByteArray(order_uid.0),
                    kind: match fee_policy {
                        FeePolicy::PriceImprovement {
                            factor,
                            max_volume_factor,
                        } => database::fee_policies::FeePolicyKind::PriceImprovement {
                            price_improvement_factor: factor,
                            max_volume_factor,
                        },
                        FeePolicy::Volume { factor } => {
                            database::fee_policies::FeePolicyKind::Volume { factor }
                        }
                    },
                };
                database::fee_policies::insert(&mut ex, fee_policy_dto)
                    .await
                    .context("fee_policies::insert")?;
            }
        }
        ex.commit().await.context("commit")
    }
}

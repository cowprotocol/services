use {
    crate::{database::Postgres, driver_model::solve::FeePolicy, protocol::fee},
    anyhow::Context,
    database::{auction::AuctionId, byte_array::ByteArray},
    model::{auction::Auction, order::OrderUid},
    std::sync::Arc,
    tracing::Instrument,
};

pub mod cli;

pub struct Persistence {
    s3: Option<s3::Uploader>,
    postgres: Arc<Postgres>,
}

impl Persistence {
    pub async fn new(config: Option<s3::Config>, postgres: Arc<Postgres>) -> Self {
        Self {
            s3: match config {
                Some(config) => Some(s3::Uploader::new(config).await),
                None => None,
            },
            postgres,
        }
    }

    /// Persists the given auction in a background task.
    pub fn store_auction(&self, id: AuctionId, instance: &Auction) {
        if let Some(uploader) = self.s3.clone() {
            let instance = instance.clone();
            tokio::spawn(
                async move {
                    match uploader.upload(id.to_string(), &instance).await {
                        Ok(key) => {
                            tracing::info!(?key, "uploaded auction to s3");
                        }
                        Err(err) => {
                            tracing::warn!(?err, "failed to upload auction to s3");
                        }
                    }
                }
                .instrument(tracing::Span::current()),
            );
        }
    }

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

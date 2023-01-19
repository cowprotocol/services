use super::instance_creation::{InstanceCreator, Instances};
use crate::{s3_instance_upload::S3InstanceUploader, solver::Auction};
use model::auction::AuctionId;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{Instrument, Span};

/// To `Driver` every http solver is presented as an individual `Solver` implementor. Internally
/// http solvers share the same data that is needed to create the instance for the same auction. In
/// order to waste less resources we create the instance once per auction in this component.
pub struct SharedInstanceCreator {
    creator: InstanceCreator,
    uploader: Option<Arc<S3InstanceUploader>>,
    last: Mutex<Option<Cache>>,
}

struct Cache {
    run: u64,
    // Arc because the instance data is big and only needs to be read.
    instances: Arc<Instances>,
}

impl SharedInstanceCreator {
    pub fn new(creator: InstanceCreator, uploader: Option<Arc<S3InstanceUploader>>) -> Self {
        Self {
            creator,
            uploader,
            last: Default::default(),
        }
    }

    /// The first call for a new run id creates the instance and stores it in a cache. Subsequent
    /// calls copy the instance from the cache (and block until the first call completes).
    pub async fn get_instances(&self, auction: Auction) -> Arc<Instances> {
        let mut guard = self.last.lock().await;
        let cache: &Cache = match guard.as_ref() {
            Some(cache) if cache.run == auction.run => cache,
            _ => {
                let instances = Arc::new(
                    self.creator
                        .prepare_instances(
                            auction.id,
                            auction.run,
                            auction.orders.clone(),
                            auction.liquidity.clone(),
                            auction.gas_price,
                            &auction.external_prices,
                        )
                        .await,
                );
                self.upload_instance_in_background(auction.id, instances.clone());
                *guard = Some(Cache {
                    run: auction.run,
                    instances,
                });
                // Unwrap because we just assigned Some.
                guard.as_ref().unwrap()
            }
        };
        cache.instances.clone()
    }

    // Happens in a task to not delay solving.
    fn upload_instance_in_background(&self, id: AuctionId, instances: Arc<Instances>) {
        if let Some(uploader) = &self.uploader {
            let uploader = uploader.clone();
            let task = async move {
                let auction = match serde_json::to_vec(&instances.plain) {
                    Ok(auction) => auction,
                    Err(err) => {
                        tracing::error!(?err, "encode auction for instance upload");
                        return;
                    }
                };
                std::mem::drop(instances);
                if let Err(err) = uploader.upload_instance(id, &auction).await {
                    tracing::error!(%id, ?err, "error uploading instance");
                }
            };
            tokio::task::spawn(task.instrument(Span::current()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::http_solver::buffers::MockBufferRetrieving;
    use primitive_types::U256;
    use shared::token_info::{MockTokenInfoFetching, TokenInfo};

    #[tokio::test]
    async fn cache_ok() {
        let mut token_infos = MockTokenInfoFetching::new();
        token_infos.expect_get_token_infos().returning(|tokens| {
            tokens
                .iter()
                .map(|token| (*token, TokenInfo::default()))
                .collect()
        });
        let mut buffer_retriever = MockBufferRetrieving::new();
        buffer_retriever.expect_get_buffers().returning(|tokens| {
            tokens
                .iter()
                .map(|token| (*token, Ok(U256::zero())))
                .collect()
        });
        let creator = InstanceCreator {
            native_token: Default::default(),
            token_info_fetcher: Arc::new(token_infos),
            buffer_retriever: Arc::new(buffer_retriever),
            market_makable_token_list: Default::default(),
            environment_metadata: Default::default(),
        };
        let shared = SharedInstanceCreator::new(creator, None);

        // Query the cache a couple of times with different auction and run ids. We check whether
        // the inner instance creator has been called by comparing the size of orders vec in the
        // model.

        let mut auction = Auction {
            id: 0,
            run: 0,
            orders: vec![],
            ..Default::default()
        };
        let instance = shared.get_instances(auction.clone()).await;
        assert_eq!(instance.plain.orders.len(), 0);

        // Size stays the same even though auction has one more order because cached result is used
        // because id in cache.
        auction.orders.push(Default::default());
        let instance = shared.get_instances(auction.clone()).await;
        assert_eq!(instance.plain.orders.len(), 0);

        // Size changes because id changes.
        auction.id = 1;
        auction.run = 1;
        let instance = shared.get_instances(auction).await;
        assert_eq!(instance.plain.orders.len(), 1);
    }
}

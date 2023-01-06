use super::{instance_creation::InstanceCreator, settlement::SettlementContext};
use crate::{s3_instance_upload::S3InstanceUploader, solver::Auction};
use model::auction::AuctionId;
use shared::http_solver::model::BatchAuctionModel;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    instances: Instances,
}

#[derive(Copy, Clone)]
pub enum InstanceType {
    Plain,
    /// without orders that are not connected to the fee token
    Filtered,
}

struct Instances {
    plain: Instance,
    filtered: Instance,
}

#[derive(Clone)]
pub struct Instance {
    pub model: BatchAuctionModel,
    pub context: SettlementContext,
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
    pub async fn get_instance(&self, auction: Auction, instance: InstanceType) -> Instance {
        let mut guard = self.last.lock().await;
        let cache: &Cache = match guard.as_ref() {
            Some(cache) if cache.run == auction.run => cache,
            _ => {
                let id = auction.id;
                let run = auction.run;
                let instances = self.create_instances(auction).await;
                self.upload_instance_in_background(id, &instances.plain.model);
                *guard = Some(Cache { run, instances });
                // Unwrap because we just assigned Some.
                guard.as_ref().unwrap()
            }
        };
        cache.get_instance(instance)
    }

    // Happens in a task to not delay solving.
    fn upload_instance_in_background(&self, id: AuctionId, model: &BatchAuctionModel) {
        if let Some(uploader) = &self.uploader {
            let model = model.clone();
            let uploader = uploader.clone();
            let task = async move {
                let auction = match serde_json::to_vec(&model) {
                    Ok(auction) => auction,
                    Err(err) => {
                        tracing::error!(?err, "encode auction for instance upload");
                        return;
                    }
                };
                if let Err(err) = uploader.upload_instance(id, auction).await {
                    tracing::error!(%id, ?err, "error uploading instance");
                }
            };
            tokio::task::spawn(task);
        }
    }
}

impl Cache {
    fn get_instance(&self, instance: InstanceType) -> Instance {
        match instance {
            InstanceType::Plain => self.instances.plain.clone(),
            InstanceType::Filtered => self.instances.filtered.clone(),
        }
    }
}

impl SharedInstanceCreator {
    async fn create_instances(&self, auction: Auction) -> Instances {
        let prepare_model = |filter: bool| {
            let auction = &auction;
            async move {
                let (model, context) = self
                    .creator
                    .prepare_model(
                        auction.id,
                        auction.run,
                        auction.orders.clone(),
                        auction.liquidity.clone(),
                        auction.gas_price,
                        &auction.external_prices,
                        filter,
                    )
                    .await;
                Instance { model, context }
            }
        };
        // TODO; change prepare_model to do both in one future
        let (plain, filtered) =
            futures::future::join(prepare_model(false), prepare_model(true)).await;
        Instances { plain, filtered }
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
        let instance = shared
            .get_instance(auction.clone(), InstanceType::Plain)
            .await;
        assert_eq!(instance.model.orders.len(), 0);

        // Size stays the same even though auction has one more order because cached result is used
        // because id in cache.
        auction.orders.push(Default::default());
        let instance = shared
            .get_instance(auction.clone(), InstanceType::Plain)
            .await;
        assert_eq!(instance.model.orders.len(), 0);

        // Size changes because id changes.
        auction.id = 1;
        auction.run = 1;
        let instance = shared.get_instance(auction, InstanceType::Plain).await;
        assert_eq!(instance.model.orders.len(), 1);
    }
}

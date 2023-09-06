use {
    super::instance_creation::{InstanceCreator, Instances},
    crate::{s3_instance_upload::S3InstanceUploader, solver::Auction},
    model::auction::AuctionId,
    once_cell::sync::OnceCell,
    prometheus::IntCounterVec,
    std::sync::Arc,
    tokio::sync::Mutex,
    tracing::{Instrument, Span},
};

/// To `Driver` every http solver is presented as an individual `Solver`
/// implementor. Internally http solvers share the same data that is needed to
/// create the instance for the same auction. In order to waste less resources
/// we create the instance once per auction in this component.
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

    /// The first call for a new run id creates the instance and stores it in a
    /// cache. Subsequent calls copy the instance from the cache (and block
    /// until the first call completes).
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
                            auction.orders,
                            auction.liquidity,
                            auction.gas_price,
                            &auction.external_prices,
                            auction.balances,
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

                let label = match uploader.upload_instance(id, &auction).await {
                    Ok(()) => "success",
                    Err(err) => {
                        tracing::warn!(%id, ?err, "error uploading instance");
                        "failure"
                    }
                };
                Metrics::get()
                    .instance_cache_uploads
                    .with_label_values(&[label])
                    .inc();
            };
            tokio::task::spawn(task.instrument(Span::current()));
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// Auction filtered orders grouped by class.
    #[metric(labels("result"))]
    instance_cache_uploads: IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        static INIT: OnceCell<&'static Metrics> = OnceCell::new();
        INIT.get_or_init(|| {
            let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();
            for result in ["success", "failure"] {
                metrics
                    .instance_cache_uploads
                    .with_label_values(&[result])
                    .reset();
            }
            metrics
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::solver::http_solver::buffers::MockBufferRetrieving,
        contracts::{dummy_contract, WETH9},
        model::order::{Order, OrderData},
        primitive_types::U256,
        shared::token_info::{MockTokenInfoFetching, TokenInfo},
    };

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
            native_token: dummy_contract!(WETH9, [0x00; 20]),
            ethflow_contract: None,
            token_info_fetcher: Arc::new(token_infos),
            buffer_retriever: Arc::new(buffer_retriever),
            market_makable_token_list: Default::default(),
            environment_metadata: Default::default(),
        };
        let shared = SharedInstanceCreator::new(creator, None);

        // Query the cache a couple of times with different auction and run ids. We
        // check whether the inner instance creator has been called by comparing
        // the size of orders vec in the model.

        let mut auction = Auction::default();
        let instance = shared.get_instances(auction.clone()).await;
        assert_eq!(instance.plain.orders.len(), 0);

        // Size stays the same even though auction has one more order because cached
        // result is used because id in cache.
        auction.orders.push(Order {
            data: OrderData {
                sell_amount: 1.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
        });
        auction.balances = crate::order_balance_filter::max_balance(&auction.orders);
        let instance = shared.get_instances(auction.clone()).await;
        assert_eq!(instance.plain.orders.len(), 0);

        // Size changes because id changes.
        auction.id = 1;
        auction.run = 1;
        let instance = shared.get_instances(auction).await;
        assert_eq!(instance.plain.orders.len(), 1);
    }
}

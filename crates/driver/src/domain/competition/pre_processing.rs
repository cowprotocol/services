use {
    super::{Auction, Order, auction, order},
    crate::{
        domain::{eth, liquidity},
        infra::{self, observe::metrics},
        util::Bytes,
    },
    chrono::Utc,
    futures::{
        FutureExt,
        future::{BoxFuture, join_all},
    },
    itertools::Itertools,
    model::{order::OrderKind, signature::Signature},
    shared::signature_validator::SignatureValidating,
    std::{
        collections::HashMap,
        future::Future,
        sync::{Arc, Mutex},
    },
    tap::TapFallible,
};

type Shared<T> = futures::future::Shared<BoxFuture<'static, T>>;
type BalanceGroup = (order::Trader, eth::TokenAddress, order::SellTokenBalance);
type Balances = HashMap<BalanceGroup, order::SellAmount>;

/// Tasks for fetching data needed to properly process auctions.
/// These are shared by all connected solvers.
/// The reason why we have 1 task per piece of data instead of 1 task
/// that returns all the data is that this allows for better composition.
/// For example if you have some logic that only depends on one or 2 of the
/// tasks you can simply await only those and already start with your new task
/// while the third task can continue in the background.
#[derive(Clone, Debug)]
pub struct DataFetchingTasks {
    pub balances: Shared<Arc<Balances>>,
    pub app_data: Shared<Arc<HashMap<order::app_data::AppDataHash, app_data::ValidatedAppData>>>,
    pub cow_amm_orders: Shared<Arc<Vec<Order>>>,
    pub liquidity: Shared<Arc<Vec<liquidity::Liquidity>>>,
}

/// All the components used for fetching the necessary data.
pub struct Utilities {
    eth: infra::Ethereum,
    signature_validator: Arc<dyn SignatureValidating>,
    app_data_retriever: Option<order::app_data::AppDataRetriever>,
    liquidity_fetcher: infra::liquidity::Fetcher,
    disable_access_list_simulation: bool,
}

impl std::fmt::Debug for Utilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Utilities").finish()
    }
}

/// All the data used to ensure that we only run 1 instance of
/// [`DataFetchingTasks`] per auction shared by all the connected solvers.
#[derive(Debug)]
struct ControlBlock {
    /// Auction for which the data aggregation task was spawned.
    auction: auction::Id,
    /// Data aggregation task.
    tasks: DataFetchingTasks,
}

#[derive(Debug)]
pub struct DataAggregator {
    utilities: Arc<Utilities>,
    control: Mutex<ControlBlock>,
}

impl DataAggregator {
    /// Aggregates all the data that is needed to pre-process the given auction.
    /// Uses a shared futures internally to make sure that the works happens
    /// only once for all connected solvers to share.
    pub fn start_or_get_tasks_for_auction(&self, auction: Arc<Auction>) -> DataFetchingTasks {
        let new_id = auction
            .id()
            .expect("auctions used for quoting do not have to be prioritized");

        let mut lock = self.control.lock().unwrap();
        let current_id = lock.auction;
        if new_id.0 < current_id.0 {
            tracing::error!(?current_id, ?new_id, "received an outdated auction");
        }
        if current_id.0 == new_id.0 {
            tracing::debug!("await running data aggregation task");
            return lock.tasks.clone();
        }

        let tasks = self.assemble_tasks(auction);

        tracing::debug!("started new data aggregation task");
        lock.auction = new_id;
        lock.tasks = tasks.clone();

        tasks
    }

    pub fn new(
        eth: infra::Ethereum,
        app_data_retriever: Option<order::app_data::AppDataRetriever>,
        liquidity_fetcher: infra::liquidity::Fetcher,
        disable_access_list_simulation: bool,
    ) -> Self {
        let signature_validator = shared::signature_validator::validator(
            eth.web3(),
            shared::signature_validator::Contracts {
                settlement: eth.contracts().settlement().clone(),
                signatures: eth.contracts().signatures().clone(),
                vault_relayer: eth.contracts().vault_relayer().0,
            },
        );

        Self {
            utilities: Arc::new(Utilities {
                eth,
                signature_validator,
                app_data_retriever,
                liquidity_fetcher,
                disable_access_list_simulation,
            }),
            control: Mutex::new(ControlBlock {
                auction: auction::Id(0),
                tasks: DataFetchingTasks {
                    balances: futures::future::pending().boxed().shared(),
                    app_data: futures::future::pending().boxed().shared(),
                    cow_amm_orders: futures::future::pending().boxed().shared(),
                    liquidity: futures::future::pending().boxed().shared(),
                },
            }),
        }
    }

    fn assemble_tasks(&self, auction: Arc<Auction>) -> DataFetchingTasks {
        let balances =
            Self::spawn_shared(Arc::clone(&self.utilities).fetch_balances(Arc::clone(&auction)));

        let app_data = Self::spawn_shared(
            Arc::clone(&self.utilities).collect_orders_app_data(Arc::clone(&auction)),
        );

        let cow_amm_orders =
            Self::spawn_shared(Arc::clone(&self.utilities).cow_amm_orders(Arc::clone(&auction)));

        let liquidity =
            Self::spawn_shared(Arc::clone(&self.utilities).fetch_liquidity(Arc::clone(&auction)));

        DataFetchingTasks {
            balances,
            app_data,
            cow_amm_orders,
            liquidity,
        }
    }

    /// Spawns an async task and returns a `Shared` handle to its result.
    /// Errors are not handled as we are deferring all error handling to where
    /// the shared future is awaited.
    /// The future is being started immediately and polled in the background so
    /// the caller doen't have to manage data dependencies directly.
    fn spawn_shared<T>(fut: impl Future<Output = T> + Send + 'static) -> Shared<T>
    where
        T: Send + Sync + Clone + 'static,
    {
        let shared = fut.boxed().shared();

        // Start the computation in the background
        tokio::spawn(shared.clone());

        shared
    }
}

impl Utilities {
    /// Fetches the tradable balance for every order owner.
    async fn fetch_balances(self: Arc<Self>, auction: Arc<Auction>) -> Arc<Balances> {
        let _timer = metrics::get().processing_stage_timer("fetch_balances");
        let ethereum = self.eth.with_metric_label("orderBalances".into());
        let mut tokens: HashMap<_, _> = Default::default();
        // Collect trader/token/source/interaction tuples for fetching available
        // balances. Note that we are pessimistic here, if a trader is selling
        // the same token with the same source in two different orders using a
        // different set of pre-interactions, then we fetch the balance as if no
        // pre-interactions were specified. This is done to avoid creating
        // dependencies between orders (i.e. order 1 is required for executing
        // order 2) which we currently cannot express with the solver interface.
        let traders = auction
            .orders
            .iter()
            .chunk_by(|order| (order.trader(), order.sell.token, order.sell_token_balance))
            .into_iter()
            .map(|((trader, token, source), mut orders)| {
                let first = orders.next().expect("group contains at least 1 order");
                let mut others = orders;
                tokens.entry(token).or_insert_with(|| ethereum.erc20(token));
                if others.all(|order| order.pre_interactions == first.pre_interactions) {
                    (trader, token, source, &first.pre_interactions[..])
                } else {
                    (trader, token, source, Default::default())
                }
            })
            .collect::<Vec<_>>();

        let balances = join_all(traders.into_iter().map(
            |(trader, token, source, interactions)| {
                let token_contract = tokens.get(&token);
                let token_contract = token_contract.expect("all tokens were created earlier");
                let fetch_balance = token_contract.tradable_balance(
                    trader.into(),
                    source,
                    interactions,
                    self.disable_access_list_simulation,
                );

                async move {
                    let balance = fetch_balance.await;
                    (
                        (trader, token, source),
                        balance.map(order::SellAmount::from).ok(),
                    )
                }
            },
        ))
        .await
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
        .collect();

        Arc::new(balances)
    }

    /// Fetches the app data for all orders in the auction.
    /// Returns a map from app data hash to the fetched app data.
    async fn collect_orders_app_data(
        self: Arc<Self>,
        auction: Arc<Auction>,
    ) -> Arc<HashMap<order::app_data::AppDataHash, app_data::ValidatedAppData>> {
        let Some(app_data_retriever) = &self.app_data_retriever else {
            return Default::default();
        };

        let _timer = metrics::get().processing_stage_timer("fetch_app_data");

        let app_data = join_all(
            auction
                .orders
                .iter()
                .map(|order| order.app_data.hash())
                .unique()
                .map(|app_data_hash| {
                    let app_data_retriever = app_data_retriever.clone();
                    async move {
                        let fetched_app_data = app_data_retriever
                            .get(&app_data_hash)
                            .await
                            .tap_err(|err| {
                                tracing::warn!(?app_data_hash, ?err, "failed to fetch app data");
                            })
                            .ok()
                            .flatten();

                        (app_data_hash, fetched_app_data)
                    }
                }),
        )
        .await
        .into_iter()
        .filter_map(|(app_data_hash, app_data)| app_data.map(|app_data| (app_data_hash, app_data)))
        .collect::<HashMap<_, _>>();

        Arc::new(app_data)
    }

    async fn cow_amm_orders(self: Arc<Self>, auction: Arc<Auction>) -> Arc<Vec<Order>> {
        let _timer = metrics::get().processing_stage_timer("cow_amm_orders");
        let cow_amms = self.eth.contracts().cow_amm_registry().amms().await;
        let domain_separator = self.eth.contracts().settlement_domain_separator();
        let domain_separator = model::DomainSeparator(domain_separator.0);
        let validator = self.signature_validator.as_ref();
        let results: Vec<_> = futures::future::join_all(
            cow_amms
                .into_iter()
                // Only generate orders for cow amms the auction told us about.
                // Otherwise the solver would expect the order to get surplus but
                // the autopilot would actually not count it.
                .filter(|amm| auction.surplus_capturing_jit_order_owners.contains(&eth::Address(*amm.address())))
                // Only generate orders where the auction provided the required
                // reference prices. Otherwise there will be an error during the
                // surplus calculation which will also result in 0 surplus for
                // this order.
                .filter_map(|amm| {
                    let prices = amm
                        .traded_tokens()
                        .iter()
                        .map(|t| {
                            auction.tokens
                                .get(eth::TokenAddress(eth::ContractAddress(*t)))
                                .price
                                .map(|p| p.0.0)
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some((amm, prices))
                })
                .map(|(cow_amm, prices)| async move {
                    let order = cow_amm.validated_template_order(
                        prices,
                        validator,
                        &domain_separator
                    ).await;
                    (*cow_amm.address(), order)
                }),
        )
        .await;

        // Convert results to domain format.
        let domain_separator = model::DomainSeparator(domain_separator.0);
        let orders: Vec<_> = results
            .into_iter()
            .filter_map(|(amm, result)| match result {
                Ok(template) => Some(Order {
                    uid: template.order.uid(&domain_separator, &amm).0.into(),
                    receiver: template.order.receiver.map(|addr| addr.into()),
                    created: u32::try_from(Utc::now().timestamp())
                        .unwrap_or(u32::MIN)
                        .into(),
                    valid_to: template.order.valid_to.into(),
                    buy: eth::Asset {
                        amount: template.order.buy_amount.into(),
                        token: template.order.buy_token.into(),
                    },
                    sell: eth::Asset {
                        amount: template.order.sell_amount.into(),
                        token: template.order.sell_token.into(),
                    },
                    kind: order::Kind::Limit,
                    side: template.order.kind.into(),
                    app_data: order::app_data::AppDataHash(Bytes(template.order.app_data.0)).into(),
                    buy_token_balance: template.order.buy_token_balance.into(),
                    sell_token_balance: template.order.sell_token_balance.into(),
                    partial: match template.order.partially_fillable {
                        true => order::Partial::Yes {
                            available: match template.order.kind {
                                OrderKind::Sell => order::TargetAmount(template.order.sell_amount),
                                OrderKind::Buy => order::TargetAmount(template.order.buy_amount),
                            },
                        },
                        false => order::Partial::No,
                    },
                    pre_interactions: template
                        .pre_interactions
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    post_interactions: template
                        .post_interactions
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    signature: match template.signature {
                        Signature::Eip1271(bytes) => order::Signature {
                            scheme: order::signature::Scheme::Eip1271,
                            data: Bytes(bytes),
                            signer: amm.into(),
                        },
                        _ => {
                            tracing::warn!(
                                signature = ?template.signature,
                                ?amm,
                                "signature for cow amm order has incorrect scheme"
                            );
                            return None;
                        }
                    },
                    protocol_fees: vec![],
                    quote: None,
                }),
                Err(err) => {
                    tracing::warn!(?err, ?amm, "failed to generate template order for cow amm");
                    None
                }
            })
            .collect();

        if !orders.is_empty() {
            tracing::debug!(?orders, "generated cow amm template orders");
        }

        Arc::new(orders)
    }

    async fn fetch_liquidity(
        self: Arc<Self>,
        auction: Arc<Auction>,
    ) -> Arc<Vec<liquidity::Liquidity>> {
        let _timer = metrics::get().processing_stage_timer("fetch_liquidity");
        let pairs = auction.liquidity_pairs();
        Arc::new(
            self.liquidity_fetcher
                .fetch(&pairs, infra::liquidity::AtBlock::Latest)
                .await,
        )
    }
}

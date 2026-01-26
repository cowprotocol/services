use {
    super::{Auction, Order, order},
    crate::{
        domain::{
            competition::order::{SellTokenBalance, app_data::AppData},
            cow_amm,
            eth,
            liquidity,
        },
        infra::{self, api::routes::solve::dto::SolveRequest, observe::metrics, tokens},
        util::Bytes,
    },
    anyhow::{Context, Result},
    chrono::Utc,
    futures::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered},
    hyper::body::Bytes as RequestBytes,
    itertools::Itertools,
    model::{
        interaction::InteractionData,
        order::{OrderKind, SellTokenSource},
        signature::Signature,
    },
    shared::{
        account_balances::{BalanceFetching, Query},
        price_estimation::trade_verifier::balance_overrides::BalanceOverrideRequest,
        signature_validator::SignatureValidating,
    },
    std::{collections::HashMap, future::Future, sync::Arc, time::Duration},
    tokio::sync::Mutex,
    tracing::Instrument,
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
    pub auction: Shared<Arc<Auction>>,
    pub balances: Shared<Arc<Balances>>,
    pub app_data:
        Shared<Arc<HashMap<order::app_data::AppDataHash, Arc<app_data::ValidatedAppData>>>>,
    pub cow_amm_orders: Shared<Arc<Vec<Order>>>,
    pub liquidity: Shared<Arc<Vec<liquidity::Liquidity>>>,
}

/// All the components used for fetching the necessary data.
pub struct Utilities {
    eth: infra::Ethereum,
    signature_validator: Arc<dyn SignatureValidating>,
    app_data_retriever: Option<order::app_data::AppDataRetriever>,
    liquidity_fetcher: infra::liquidity::Fetcher,
    tokens: tokens::Fetcher,
    balance_fetcher: Arc<dyn BalanceFetching>,
    cow_amm_cache: Option<cow_amm::Cache>,
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
    solve_request: RequestBytes,
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
    pub async fn start_or_get_tasks_for_auction(
        &self,
        request: RequestBytes,
    ) -> Result<DataFetchingTasks> {
        let mut lock = self.control.lock().await;
        let current_auction = &lock.solve_request;

        // The autopilot ensures that all drivers receive identical
        // requests per auction. That means we can use the significantly
        // cheaper string comparison instead of parsing the JSON to compare
        // the auction ids.
        if request == current_auction {
            let id = lock.tasks.auction.clone().await.id;
            init_auction_id_in_span(id.map(|i| i.0));
            tracing::debug!("await running data aggregation task");
            return Ok(lock.tasks.clone());
        }

        let tasks = self.assemble_tasks(request.clone()).await?;

        tracing::debug!("started new data aggregation task");
        lock.solve_request = request;
        lock.tasks = tasks.clone();

        Ok(tasks)
    }

    pub fn new(
        eth: infra::Ethereum,
        app_data_retriever: Option<order::app_data::AppDataRetriever>,
        liquidity_fetcher: infra::liquidity::Fetcher,
        tokens: tokens::Fetcher,
        balance_fetcher: Arc<dyn BalanceFetching>,
    ) -> Self {
        let signature_validator = shared::signature_validator::validator(
            eth.web3(),
            shared::signature_validator::Contracts {
                settlement: eth.contracts().settlement().clone(),
                vault_relayer: eth.contracts().vault_relayer().0,
                signatures: eth.contracts().signatures().clone(),
            },
            eth.balance_overrider(),
        );

        let cow_amm_helper_by_factory = eth
            .contracts()
            .cow_amm_helper_by_factory()
            .iter()
            .map(|(factory, helper)| (factory.0, helper.0))
            .collect();
        let cow_amm_cache =
            cow_amm::Cache::new(eth.web3().alloy.clone(), cow_amm_helper_by_factory);

        Self {
            utilities: Arc::new(Utilities {
                eth,
                signature_validator,
                app_data_retriever,
                liquidity_fetcher,
                tokens,
                balance_fetcher,
                cow_amm_cache,
            }),
            control: Mutex::new(ControlBlock {
                solve_request: Default::default(),
                tasks: DataFetchingTasks {
                    auction: futures::future::pending().boxed().shared(),
                    balances: futures::future::pending().boxed().shared(),
                    app_data: futures::future::pending().boxed().shared(),
                    cow_amm_orders: futures::future::pending().boxed().shared(),
                    liquidity: futures::future::pending().boxed().shared(),
                },
            }),
        }
    }

    async fn assemble_tasks(&self, request: RequestBytes) -> Result<DataFetchingTasks> {
        let auction = self.utilities.parse_request(request).await?;

        let balances =
            Self::spawn_shared(Arc::clone(&self.utilities).fetch_balances(Arc::clone(&auction)));

        let app_data = Self::spawn_shared(
            Arc::clone(&self.utilities).collect_orders_app_data(Arc::clone(&auction)),
        );

        let cow_amm_orders =
            Self::spawn_shared(Arc::clone(&self.utilities).cow_amm_orders(Arc::clone(&auction)));

        let liquidity =
            Self::spawn_shared(Arc::clone(&self.utilities).fetch_liquidity(Arc::clone(&auction)));

        Ok(DataFetchingTasks {
            auction: futures::future::ready(auction).boxed().shared(),
            balances,
            app_data,
            cow_amm_orders,
            liquidity,
        })
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
        let shared = fut.instrument(tracing::Span::current()).boxed().shared();

        // Start the computation in the background
        tokio::spawn(shared.clone());

        shared
    }
}

impl Utilities {
    /// Parses the JSON body of the `/solve` request during the unified
    /// auction pre-processing since eagerly deserializing these requests
    /// is surprisingly costly because their are so big.
    async fn parse_request(&self, solve_request: RequestBytes) -> Result<Arc<Auction>> {
        let auction_dto: SolveRequest = {
            let _timer = metrics::get().processing_stage_timer("parse_dto");
            let _timer2 =
                observe::metrics::metrics().on_auction_overhead_start("driver", "parse_dto");
            // deserialization takes tens of milliseconds so run it on a blocking task
            tokio::task::spawn_blocking(move || {
                serde_json::from_slice(&solve_request).context("could not parse solve request")
            })
            .await
            .context("failed to await blocking task")??
        };

        // now that we finally know the auction id we can set it in the span
        init_auction_id_in_span(Some(auction_dto.id()));

        let auction_domain = {
            let _timer = metrics::get().processing_stage_timer("convert_to_domain");
            let _timer2 = observe::metrics::metrics()
                .on_auction_overhead_start("driver", "convert_to_domain");
            let app_data = self
                .app_data_retriever
                .as_ref()
                .map(|retriever| retriever.get_cached())
                .unwrap_or_default();
            let auction = auction_dto
                .into_domain(&self.eth, &self.tokens, app_data)
                .await
                .context("could not convert auction DTO to domain type")?;
            Arc::new(auction)
        };

        Ok(auction_domain)
    }

    /// Fetches the tradable balance for every order owner.
    async fn fetch_balances(self: Arc<Self>, auction: Arc<Auction>) -> Arc<Balances> {
        let _timer = metrics::get().processing_stage_timer("fetch_balances");
        let _timer2 =
            observe::metrics::metrics().on_auction_overhead_start("driver", "fetch_balances");

        // Collect trader/token/source/interaction tuples for fetching available
        // balances. Note that we are pessimistic here, if a trader is selling
        // the same token with the same source in two different orders using a
        // different set of pre-interactions, then we fetch the balance as if no
        // pre-interactions were specified. This is done to avoid creating
        // dependencies between orders (i.e. order 1 is required for executing
        // order 2) which we currently cannot express with the solver interface.
        let queries = auction
            .orders
            .iter()
            .chunk_by(|order| (order.trader(), order.sell.token, order.sell_token_balance))
            .into_iter()
            .map(|((trader, token, source), mut orders)| {
                let first = orders.next().expect("group contains at least 1 order");
                let mut others = orders;
                let all_setups_equal = others.all(|order| {
                    order.pre_interactions == first.pre_interactions
                        && order.app_data.flashloan() == first.app_data.flashloan()
                });
                Query {
                    owner: trader.0,
                    token: token.0.0,
                    source: match source {
                        SellTokenBalance::Erc20 => SellTokenSource::Erc20,
                        SellTokenBalance::Internal => SellTokenSource::Internal,
                        SellTokenBalance::External => SellTokenSource::External,
                    },
                    interactions: if all_setups_equal {
                        first
                            .pre_interactions
                            .iter()
                            .map(|i| InteractionData {
                                target: i.target,
                                value: i.value.0,
                                call_data: i.call_data.0.clone(),
                            })
                            .collect()
                    } else {
                        Vec::default()
                    },
                    balance_override: if all_setups_equal {
                        first
                            .app_data
                            .flashloan()
                            .map(|loan| BalanceOverrideRequest {
                                token: loan.token,
                                amount: loan.amount,
                                holder: loan.receiver,
                            })
                    } else {
                        None
                    },
                }
            })
            .collect::<Vec<_>>();

        let balances = self.balance_fetcher.get_balances(&queries).await;

        let result: HashMap<_, _> = queries
            .into_iter()
            .zip(balances)
            .filter_map(|(query, balance)| {
                let balance = balance.ok()?;
                Some((
                    (
                        order::Trader(query.owner),
                        query.token.into(),
                        match query.source {
                            SellTokenSource::Erc20 => SellTokenBalance::Erc20,
                            SellTokenSource::Internal => SellTokenBalance::Internal,
                            SellTokenSource::External => SellTokenBalance::External,
                        },
                    ),
                    order::SellAmount(balance),
                ))
            })
            .collect();

        Arc::new(result)
    }

    /// Fetches the app data for all orders in the auction.
    /// Returns a map from app data hash to the fetched app data.
    async fn collect_orders_app_data(
        self: Arc<Self>,
        auction: Arc<Auction>,
    ) -> Arc<HashMap<order::app_data::AppDataHash, Arc<app_data::ValidatedAppData>>> {
        let Some(app_data_retriever) = &self.app_data_retriever else {
            return Default::default();
        };

        let _timer = metrics::get().processing_stage_timer("fetch_app_data");
        let _timer2 =
            observe::metrics::metrics().on_auction_overhead_start("driver", "fetch_app_data");

        let futures: FuturesUnordered<_> = auction
            .orders
            .iter()
            .flat_map(|order| match order.app_data {
                AppData::Full(_) => None,
                // only fetch appdata we don't already have in full
                AppData::Hash(hash) => Some(hash),
            })
            .unique()
            .map(async move |app_data_hash| {
                let fetched_app_data = app_data_retriever
                    .get_cached_or_fetch(&app_data_hash)
                    .await
                    .inspect_err(|err| {
                        tracing::warn!(?app_data_hash, ?err, "failed to fetch app data");
                    })
                    .ok()
                    .flatten();

                (app_data_hash, fetched_app_data)
            })
            .collect();

        // Only await responses for a short amount of time. Even if we don't await
        // all futures fully the remaining appdata requests will finish in background
        // tasks. That way we should have enough time to immediately fetch appdatas
        // of new orders (once the cache is filled). But we also don't run the risk
        // of stalling the driver completely until everything is fetched.
        // In practice that means the solver will only see a few appdatas in the first
        // auction after a restart. But on subsequent auctions everything should be
        // available.
        const MAX_APP_DATA_WAIT: Duration = Duration::from_millis(500);
        let app_data: HashMap<_, _> = futures
            .take_until(tokio::time::sleep(MAX_APP_DATA_WAIT))
            .filter_map(async move |(hash, json)| Some((hash, json?)))
            .collect()
            .await;

        Arc::new(app_data)
    }

    async fn cow_amm_orders(self: Arc<Self>, auction: Arc<Auction>) -> Arc<Vec<Order>> {
        let Some(ref cow_amm_cache) = self.cow_amm_cache else {
            // CoW AMMs are not configured, return empty vec
            return Default::default();
        };

        let _timer = metrics::get().processing_stage_timer("cow_amm_orders");
        let _timer2 =
            observe::metrics::metrics().on_auction_overhead_start("driver", "cow_amm_orders");

        let cow_amms = cow_amm_cache
            .get_or_create_amms(&auction.surplus_capturing_jit_order_owners)
            .await;

        let domain_separator = self.eth.contracts().settlement_domain_separator();
        let domain_separator = model::DomainSeparator(domain_separator.0);
        let validator = self.signature_validator.as_ref();

        let results: Vec<_> = futures::future::join_all(
            cow_amms
                .into_iter()
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
                                .get(&eth::TokenAddress(eth::ContractAddress(*t)))
                                .and_then(|token| token.price)
                                .map(|price| price.0.0)
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
                    uid: template.order.uid(&domain_separator, amm).0.into(),
                    receiver: template.order.receiver,
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
                            signer: amm,
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
        let _timer2 =
            observe::metrics::metrics().on_auction_overhead_start("driver", "fetch_liquidity");
        let pairs = auction.liquidity_pairs();
        Arc::new(
            self.liquidity_fetcher
                .fetch(&pairs, infra::liquidity::AtBlock::Latest)
                .await,
        )
    }
}

fn init_auction_id_in_span(id: Option<i64>) {
    let Some(id) = id else {
        return;
    };
    let current_span = tracing::Span::current();
    debug_assert!(current_span.has_field("auction_id"));
    current_span.record("auction_id", id);
}

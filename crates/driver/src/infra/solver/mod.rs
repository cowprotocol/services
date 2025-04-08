use {
    super::notify,
    crate::{
        domain::{
            competition::{
                auction::{self, Auction},
                bad_tokens,
                order::{self, Partial, Side, app_data::AppData},
                solution::{
                    self,
                    Id,
                    Interaction,
                    Solution,
                    Trade,
                    interaction::Custom,
                    trade::{Fee, Fulfillment},
                },
            },
            eth,
            liquidity,
            time::Remaining,
        },
        infra::{
            blockchain::Ethereum,
            config::file::FeeHandler,
            persistence::{Persistence, S3},
        },
        util::{self, Bytes},
    },
    anyhow::Result,
    app_data::AppDataHash,
    cached::SizedCache,
    contracts::{ERC20, HooksTrampoline},
    derive_more::{From, Into},
    model::order::{OrderData, OrderKind},
    num::BigRational,
    reqwest::header::HeaderName,
    shared::{
        addr,
        price_estimation::trade_verifier::{
            TradeVerifier,
            balance_overrides::{BalanceOverrides, detector::Detector},
        },
    },
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::Duration,
    },
    tap::TapFallible,
    thiserror::Error,
    tracing::Instrument,
};

pub mod dto;

// TODO At some point I should be checking that the names are unique, I don't
// think I'm doing that.
/// The solver name. The user can configure this to be anything that they like.
/// The name uniquely identifies each solver in case there's more than one of
/// them.
#[derive(Debug, Clone, From, Into)]
pub struct Name(pub String);

impl Name {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Slippage {
    pub relative: BigRational,
    pub absolute: Option<eth::Ether>,
}

#[derive(Clone, Copy, Debug)]
pub enum Liquidity {
    /// Liquidity should be fetched and included in the auction sent to this
    /// solver.
    Fetch,
    /// The solver does not need liquidity, so fetching can be skipped for this
    /// solver.
    Skip,
}

#[derive(Clone, Copy, Debug)]
pub struct Timeouts {
    /// Maximum time allocated for http request/reponse to propagate through
    /// network.
    pub http_delay: chrono::Duration,
    /// Maximum time allocated for solver engines to return the solutions back
    /// to the driver, in percentage of total driver deadline.
    pub solving_share_of_deadline: util::Percent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManageNativeToken {
    /// If true wraps ETH address
    pub wrap_address: bool,
    /// If true inserts unwrap interactions
    pub insert_unwraps: bool,
}

/// Solvers are controlled by the driver. Their job is to search for solutions
/// to auctions. They do this in various ways, often by analyzing different AMMs
/// on the Ethereum blockchain.
#[derive(Debug, Clone)]
pub struct Solver {
    client: reqwest::Client,
    config: Config,
    eth: Ethereum,
    persistence: Persistence,
    verifier: Arc<TradeVerifier>,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The endpoint of the solver, including the path (commonly "/solve").
    pub endpoint: url::Url,
    pub name: Name,
    /// The acceptable slippage for this solver.
    pub slippage: Slippage,
    /// Whether or not liquidity is used by this solver.
    pub liquidity: Liquidity,
    /// The private key of this solver, used for settlement submission.
    pub account: ethcontract::Account,
    /// How much time to spend for each step of the solving and competition.
    pub timeouts: Timeouts,
    /// HTTP headers that should be added to every request.
    pub request_headers: HashMap<String, String>,
    /// Determines whether the `solver` or the `driver` handles the fees
    pub fee_handler: FeeHandler,
    /// Use limit orders for quoting
    /// TODO: Remove once all solvers are moved to use limit orders for quoting
    pub quote_using_limit_orders: bool,
    pub merge_solutions: SolutionMerging,
    /// S3 configuration for storing the auctions in the form they are sent to
    /// the solver engine
    pub s3: Option<S3>,
    /// Whether the native token is wrapped or not when sent to the solvers
    pub solver_native_token: ManageNativeToken,
    /// Which `tx.origin` is required to make quote verification pass.
    pub quote_tx_origin: Option<eth::Address>,
    pub response_size_limit_max_bytes: usize,
    pub bad_token_detection: BadTokenDetection,
    /// Max size of the pending settlements queue.
    pub settle_queue_size: usize,
    /// Whether flashloan hints should be sent to the solver.
    pub flashloans_enabled: bool,
}

impl Solver {
    pub async fn try_new(config: Config, eth: Ethereum) -> Result<Self> {
        let web3 = eth.web3();
        let overrides = Arc::new(BalanceOverrides {
            hardcoded: Default::default(),
            detector: Some((
                Detector::new(Arc::new(web3.clone()), 50),
                Mutex::new(SizedCache::with_size(100)),
            )),
        });

        let verifier: Arc<TradeVerifier> = Arc::new(
            TradeVerifier::new(
                web3.clone(),
                Arc::new(web3.clone()),
                Arc::new(web3.clone()),
                overrides,
                eth.current_block().clone(),
                eth.contracts().settlement().address(),
                eth.contracts().weth().address(),
                100.into(),
            )
            .await
            .unwrap(),
        );

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());

        for (key, val) in config.request_headers.iter() {
            let header_name = HeaderName::try_from(key)?;
            headers.insert(header_name, val.parse()?);
        }

        let persistence = Persistence::build(&config).await;

        Ok(Self {
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()?,
            config,
            eth,
            persistence,
            verifier,
        })
    }

    pub fn bad_token_detection(&self) -> &BadTokenDetection {
        &self.config.bad_token_detection
    }

    pub fn persistence(&self) -> Persistence {
        self.persistence.clone()
    }

    pub fn name(&self) -> &Name {
        &self.config.name
    }

    /// The slippage configuration of this solver.
    pub fn slippage(&self) -> &Slippage {
        &self.config.slippage
    }

    /// The liquidity configuration of this solver
    pub fn liquidity(&self) -> Liquidity {
        self.config.liquidity
    }

    /// The blockchain address of this solver.
    pub fn address(&self) -> eth::Address {
        self.config.account.address().into()
    }

    /// The account which should be used to sign settlements for this solver.
    pub fn account(&self) -> ethcontract::Account {
        self.config.account.clone()
    }

    /// Timeout configuration for this solver.
    pub fn timeouts(&self) -> Timeouts {
        self.config.timeouts
    }

    /// Use limit orders for quoting instead of market orders
    pub fn quote_using_limit_orders(&self) -> bool {
        self.config.quote_using_limit_orders
    }

    pub fn solution_merging(&self) -> SolutionMerging {
        self.config.merge_solutions
    }

    pub fn solver_native_token(&self) -> ManageNativeToken {
        self.config.solver_native_token
    }

    pub fn quote_tx_origin(&self) -> &Option<eth::Address> {
        &self.config.quote_tx_origin
    }

    pub fn settle_queue_size(&self) -> usize {
        self.config.settle_queue_size
    }

    /// Make a POST request instructing the solver to solve an auction.
    /// Allocates at most `timeout` time for the solving.
    pub async fn solve(
        &self,
        auction: &Auction,
        liquidity: &[liquidity::Liquidity],
    ) -> Result<Vec<Solution>, Error> {
        // Fetch the solutions from the solver.
        let weth = self.eth.contracts().weth_address();
        let auction_dto = dto::auction::new(
            auction,
            liquidity,
            weth,
            self.config.fee_handler,
            self.config.solver_native_token,
            self.config.flashloans_enabled,
            self.eth.contracts().flashloan_default_lender(),
        );
        // Only auctions with IDs are real auctions (/quote requests don't have an ID,
        // and it makes no sense to store them)
        if let Some(id) = auction.id() {
            self.persistence.archive_auction(id, &auction_dto);
        };
        let body = serde_json::to_string(&auction_dto).unwrap();
        let url = shared::url::join(&self.config.endpoint, "solve");
        super::observe::solver_request(&url, &body);
        let timeout = match auction.deadline().solvers().remaining() {
            Ok(timeout) => timeout,
            Err(_) => {
                tracing::warn!("auction deadline exceeded before sending request to solver");
                return Ok(Default::default());
            }
        };
        let mut req = self.client.post(url.clone()).body(body).timeout(timeout);
        if let Some(id) = observe::request_id::from_current_span() {
            req = req.header("X-REQUEST-ID", id);
        }
        let res = util::http::send(self.config.response_size_limit_max_bytes, req).await;
        super::observe::solver_response(&url, res.as_deref());
        let res = res?;
        let res: solvers_dto::solution::Solutions = serde_json::from_str(&res)
            .tap_err(|err| tracing::warn!(res, ?err, "failed to parse solver response"))?;
        let mut solutions = dto::Solutions::from(res).into_domain(
            auction,
            liquidity,
            weth,
            self.clone(),
            &self.config,
        )?;

        // TODO add all the reference solutions
        let mut reference_solutions: Vec<Solution> =
            futures::future::join_all(auction.orders().iter().enumerate().map(|(i, o)| {
                let verifier = self.verifier.clone();
                let web3 = self.eth.web3().clone();

                async move {
                    let AppData::Full(data) = &o.app_data else {
                        return None;
                    };
                    if data.protocol.reference_solution.is_empty() {
                        return None;
                    }
                    let reference = &data.protocol.reference_solution;
                    tracing::error!("WE GOT HERE!!");

                    // TODO hook instance
                    // let trampoline = addr!("01DcB88678aedD0C4cC9552B20F4718550250574");
                    // switch back to real address for the sepolia test
                    let trampoline = addr!("b7f8bc63bbcad18155201308c8f3540b07f84f5e");
                    let hook = HooksTrampoline::at(&web3, trampoline);
                    let reference = hook
                        .execute(
                            reference
                                .iter()
                                .map(|i| {
                                    (
                                        i.target,
                                        ethcontract::tokens::Bytes(i.call_data.clone()),
                                        i.value,
                                    )
                                })
                                .collect::<Vec<_>>(),
                        )
                        .tx
                        .data
                        .unwrap();

                    let sell_token = ERC20::at(&web3, o.sell.token.0.0);

                    let transfer_call = sell_token
                        .transfer(trampoline, o.sell.amount.0)
                        .tx
                        .data
                        .unwrap()
                        .0;

                    let res = verifier
                        .simulate_interaction(
                            o.signature.signer.into(),
                            self.config.account.address(),
                            &OrderData {
                                sell_token: o.sell.token.0.0,
                                buy_token: o.buy.token.0.0,
                                receiver: o.receiver.map(Into::into),
                                sell_amount: o.sell.amount.0,
                                buy_amount: o.buy.amount.0,
                                valid_to: o.valid_to.0,
                                app_data: AppDataHash(o.app_data.hash().0.0),
                                fee_amount: 0.into(),
                                kind: match o.side {
                                    Side::Sell => OrderKind::Sell,
                                    Side::Buy => OrderKind::Buy,
                                },
                                partially_fillable: matches!(o.partial, Partial::Yes { .. }),
                                sell_token_balance: Default::default(),
                                buy_token_balance: Default::default(),
                            },
                            o.pre_interactions
                                .iter()
                                .map(|i| shared::trade_finding::Interaction {
                                    target: i.target.0,
                                    value: i.value.0,
                                    data: i.call_data.0.clone(),
                                })
                                .collect(),
                            vec![
                                shared::trade_finding::Interaction {
                                    target: sell_token.address(),
                                    value: 0.into(),
                                    data: transfer_call.clone(),
                                },
                                shared::trade_finding::Interaction {
                                    target: trampoline,
                                    value: 0.into(),
                                    data: reference.clone().0.clone(),
                                },
                            ],
                            o.post_interactions
                                .iter()
                                .map(|i| shared::trade_finding::Interaction {
                                    target: i.target.0,
                                    value: i.value.0,
                                    data: i.call_data.0.clone(),
                                })
                                .collect(),
                        )
                        .await;

                    let (out_amount, gas) = res.unwrap();
                    tracing::error!(?out_amount, ?gas);

                    let (sell_amount, buy_amount) = match o.side {
                        Side::Sell => (o.sell.amount.0, out_amount),
                        Side::Buy => (out_amount, o.buy.amount.0),
                    };

                    let sol = Solution {
                        id: Id::new(10_000 + i as u64),
                        trades: vec![Trade::Fulfillment(Fulfillment {
                            order: o.clone(),
                            executed: o.target(),
                            fee: Fee::Dynamic(order::SellAmount(0.into())),
                        })],
                        prices: [(o.sell.token, buy_amount), (o.buy.token, sell_amount)]
                            .into_iter()
                            .collect(),
                        pre_interactions: o.pre_interactions.clone(),
                        // TODO: transfer sell amounts into trampoline
                        // TODO: adjust interactions to actually work with trampoline contract
                        // TODO: recover funds from trampoline
                        interactions: vec![
                            Interaction::Custom(Custom {
                                target: o.sell.token.into(),
                                value: 0.into(),
                                call_data: Bytes(transfer_call.clone()),
                                allowances: vec![],
                                inputs: vec![],
                                outputs: vec![],
                                internalize: false,
                            }),
                            Interaction::Custom(Custom {
                                target: trampoline.into(),
                                value: 0.into(),
                                call_data: Bytes(reference.0.clone()),
                                allowances: vec![],
                                inputs: vec![eth::Asset {
                                    token: o.sell.token,
                                    amount: sell_amount.into(),
                                }],
                                outputs: vec![eth::Asset {
                                    token: o.buy.token,
                                    amount: buy_amount.into(),
                                }],
                                internalize: false,
                            }),
                        ],
                        post_interactions: o.post_interactions.clone(),
                        solver: self.clone(),
                        weth: self.eth.contracts().weth().address().into(),
                        gas: None,
                        flashloans: vec![],
                    };
                    Some(sol)
                }
            }))
            .await
            .into_iter()
            .flatten()
            .collect();
        solutions.append(&mut reference_solutions);

        super::observe::solutions(&solutions, auction.surplus_capturing_jit_order_owners());
        Ok(solutions)
    }

    /// Make a fire and forget POST request to notify the solver about an event.
    pub fn notify(
        &self,
        auction_id: Option<auction::Id>,
        solution_id: Option<solution::Id>,
        kind: notify::Kind,
    ) {
        let body =
            serde_json::to_string(&dto::notification::new(auction_id, solution_id, kind)).unwrap();
        let url = shared::url::join(&self.config.endpoint, "notify");
        super::observe::solver_request(&url, &body);
        let mut req = self.client.post(url).body(body);
        if let Some(id) = observe::request_id::from_current_span() {
            req = req.header("X-REQUEST-ID", id);
        }
        let response_size = self.config.response_size_limit_max_bytes;
        let future = async move {
            if let Err(error) = util::http::send(response_size, req).await {
                tracing::warn!(?error, "failed to notify solver");
            }
        };
        tokio::task::spawn(future.in_current_span());
    }
}

/// Controls whether or not the driver is allowed to merge multiple solutions
/// of the same solver to produce an overall better solution.
#[derive(Debug, Clone, Copy)]
pub enum SolutionMerging {
    Allowed {
        max_orders_per_merged_solution: usize,
    },
    Forbidden,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP error: {0:?}")]
    Http(#[from] util::http::Error),
    #[error("JSON deserialization error: {0:?}")]
    Deserialize(#[from] serde_json::Error),
    #[error("solver dto error: {0}")]
    Dto(#[from] dto::Error),
}

impl Error {
    pub fn is_timeout(&self) -> bool {
        match self {
            Self::Http(util::http::Error::Response(err)) => err.is_timeout(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BadTokenDetection {
    /// Tokens that are explicitly allow- or deny-listed.
    pub tokens_supported: HashMap<eth::TokenAddress, bad_tokens::Quality>,
    pub enable_simulation_strategy: bool,
    pub enable_metrics_strategy: bool,
    pub metrics_strategy_failure_ratio: f64,
    pub metrics_strategy_required_measurements: u32,
    pub metrics_strategy_log_only: bool,
    pub metrics_strategy_token_freeze_time: Duration,
}

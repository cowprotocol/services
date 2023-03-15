use {
    super::{Asset, Order},
    crate::{
        domain::{competition::order, eth},
        infra,
        tests::{self, boundary},
    },
    ethcontract::{dyns::DynWeb3, transport::DynTransport, Web3},
    futures::Future,
    secp256k1::SecretKey,
    std::collections::HashMap,
};

/// The URL to which a post request can be made to start and stop geth
/// instances. See the `dev-geth` crate.
const DEV_GETH_PORT: &str = "8547";

// TODO Possibly might be a good idea to use an enum for tokens instead of
// &'static str

#[derive(Debug)]
pub struct Pair {
    token_a: &'static str,
    token_b: &'static str,
    contract: contracts::IUniswapLikePair,
    pool: Pool,
}

#[derive(Debug)]
pub struct Blockchain {
    pub solver_address: ethcontract::H160,
    pub solver_secret_key: SecretKey,
    pub trader_address: ethcontract::H160,
    pub trader_secret_key: SecretKey,

    pub web3: Web3<DynTransport>,
    pub tokens: HashMap<&'static str, contracts::ERC20Mintable>,
    pub weth: contracts::WETH9,
    pub settlement: contracts::GPv2Settlement,
    pub domain_separator: boundary::DomainSeparator,
    pub geth: Geth,
    pub pairs: Vec<Pair>,
    bogus_calldata: bool,
}

#[derive(Debug)]
pub struct Interaction {
    pub address: ethcontract::H160,
    pub calldata: Vec<u8>,
    pub inputs: Vec<eth::Asset>,
    pub outputs: Vec<eth::Asset>,
    pub internalize: bool,
}

/// A uniswap pool deployed as part of the blockchain setup.
#[derive(Debug, Clone, Copy)]
pub struct Pool {
    pub reserve_a: Asset,
    pub reserve_b: Asset,
}

impl Pool {
    /// Use the Uniswap constant AMM formula to calculate the output amount
    /// based on the input.
    fn out(&self, input: Asset) -> eth::U256 {
        let (input_reserve, output_reserve) = if input.token == self.reserve_a.token {
            (self.reserve_a.amount, self.reserve_b.amount)
        } else {
            (self.reserve_b.amount, self.reserve_a.amount)
        };
        output_reserve * input.amount * eth::U256::from(997)
            / (input_reserve * eth::U256::from(1000) + input.amount * eth::U256::from(997))
    }
}

#[derive(Debug)]
pub struct Fulfillment {
    pub order: Order,
    pub interactions: Vec<Interaction>,
    pub sell_amount: eth::U256,
    pub buy_amount: eth::U256,
}

impl Fulfillment {
    pub fn order_uid(
        &self,
        blockchain: &Blockchain,
        now: infra::time::Now,
    ) -> tests::boundary::OrderUid {
        self.boundary(blockchain, now).uid()
    }

    pub fn order_signature(&self, blockchain: &Blockchain, now: infra::time::Now) -> Vec<u8> {
        self.boundary(blockchain, now).signature()
    }

    fn boundary(&self, blockchain: &Blockchain, now: infra::time::Now) -> tests::boundary::Order {
        tests::boundary::Order {
            sell_token: blockchain.get_token(self.order.sell_token),
            buy_token: blockchain.get_token(self.order.buy_token),
            sell_amount: self.sell_amount,
            buy_amount: self.buy_amount,
            valid_to: u32::try_from(now.now().timestamp()).unwrap() + self.order.valid_for.0,
            user_fee: self.order.user_fee,
            side: self.order.side,
            secret_key: blockchain.trader_secret_key,
            domain_separator: blockchain.domain_separator,
            owner: blockchain.trader_address,
            partially_fillable: matches!(self.order.partial, order::Partial::Yes { .. }),
        }
    }
}

pub struct Config {
    pub pools: Vec<Pool>,
    pub bogus_calldata: bool,
    pub trader_address: eth::H160,
    pub trader_secret_key: SecretKey,
    pub solver_address: eth::H160,
    pub solver_secret_key: SecretKey,
}

impl Blockchain {
    /// Start a local geth node using the `dev-geth` crate and deploy the
    /// settlement contract, token contracts, and all supporting contracts
    /// for the settlement.
    pub async fn new(config: Config) -> Self {
        // TODO All these various deployments that are happening from the trader account
        // should be happening from the primary_account of the geth node, will do this
        // later

        let geth = Geth::new().await;
        let web3 = Web3::new(DynTransport::new(
            web3::transports::Http::new(&geth.url()).expect("valid URL"),
        ));

        let trader_account = ethcontract::Account::Offline(
            ethcontract::PrivateKey::from_slice(config.trader_secret_key.as_ref()).unwrap(),
            None,
        );

        // Use the geth account to fund the trader and the solver with ETH.
        let balance = web3
            .eth()
            .balance(primary_address(&web3).await, None)
            .await
            .unwrap();
        wait_for(
            &web3,
            web3.eth()
                .send_transaction(web3::types::TransactionRequest {
                    from: primary_address(&web3).await,
                    to: Some(config.trader_address),
                    value: Some(balance / 5),
                    ..Default::default()
                }),
        )
        .await
        .unwrap();
        wait_for(
            &web3,
            web3.eth()
                .send_transaction(web3::types::TransactionRequest {
                    from: primary_address(&web3).await,
                    to: Some(config.solver_address),
                    value: Some(balance / 5),
                    ..Default::default()
                }),
        )
        .await
        .unwrap();

        // Deploy WETH and wrap some funds in the primary account of the geth node.
        let weth = wait_for(
            &web3,
            contracts::WETH9::builder(&web3)
                .from(trader_account.clone())
                .deploy(),
        )
        .await
        .unwrap();
        wait_for(
            &web3,
            ethcontract::transaction::TransactionBuilder::new(web3.clone())
                .from(primary_account(&web3).await)
                .to(weth.address())
                .value(balance / 5)
                .send(),
        )
        .await
        .unwrap();

        // Set up the settlement contract and related contracts.
        let vault_authorizer = wait_for(
            &web3,
            contracts::BalancerV2Authorizer::builder(&web3, config.trader_address)
                .from(trader_account.clone())
                .deploy(),
        )
        .await
        .unwrap();
        let vault = wait_for(
            &web3,
            contracts::BalancerV2Vault::builder(
                &web3,
                vault_authorizer.address(),
                weth.address(),
                0.into(),
                0.into(),
            )
            .from(trader_account.clone())
            .deploy(),
        )
        .await
        .unwrap();
        let authenticator = wait_for(
            &web3,
            contracts::GPv2AllowListAuthentication::builder(&web3)
                .from(trader_account.clone())
                .deploy(),
        )
        .await
        .unwrap();
        let settlement = wait_for(
            &web3,
            contracts::GPv2Settlement::builder(&web3, authenticator.address(), vault.address())
                .from(trader_account.clone())
                .deploy(),
        )
        .await
        .unwrap();
        wait_for(
            &web3,
            authenticator
                .initialize_manager(config.trader_address)
                .from(trader_account.clone())
                .send(),
        )
        .await
        .unwrap();
        wait_for(
            &web3,
            authenticator
                .add_solver(config.solver_address)
                .from(trader_account.clone())
                .send(),
        )
        .await
        .unwrap();

        let domain_separator =
            boundary::DomainSeparator(settlement.domain_separator().call().await.unwrap().0);

        // Create the tokens needed by the pools.
        let mut tokens = HashMap::new();
        for pool in config.pools.iter() {
            if !tokens.contains_key(pool.reserve_a.token) {
                let token = wait_for(
                    &web3,
                    contracts::ERC20Mintable::builder(&web3)
                        .from(trader_account.clone())
                        .deploy(),
                )
                .await
                .unwrap();
                tokens.insert(pool.reserve_a.token, token);
            }
            if !tokens.contains_key(pool.reserve_b.token) {
                let token = wait_for(
                    &web3,
                    contracts::ERC20Mintable::builder(&web3)
                        .from(trader_account.clone())
                        .deploy(),
                )
                .await
                .unwrap();
                tokens.insert(pool.reserve_b.token, token);
            }
        }

        // Create the uniswap factory.
        let uniswap_factory = wait_for(
            &web3,
            contracts::UniswapV2Factory::builder(&web3, config.trader_address)
                .from(trader_account.clone())
                .deploy(),
        )
        .await
        .unwrap();

        // Create and fund a uniswap pair for each pool. Fund the settlement contract
        // with the same liquidity as the pool, to allow for internalized interactions.
        let mut pairs = Vec::new();
        for pool in config.pools {
            // Get token addresses.
            let token_a = if pool.reserve_a.token == "WETH" {
                weth.address()
            } else {
                tokens.get(pool.reserve_a.token).unwrap().address()
            };
            let token_b = if pool.reserve_b.token == "WETH" {
                weth.address()
            } else {
                tokens.get(pool.reserve_b.token).unwrap().address()
            };
            // Create the pair.
            wait_for(
                &web3,
                uniswap_factory
                    .create_pair(token_a, token_b)
                    .from(trader_account.clone())
                    .send(),
            )
            .await
            .unwrap();
            // Fund the pair and the settlement contract.
            let pair = contracts::IUniswapLikePair::at(
                &web3,
                uniswap_factory
                    .get_pair(token_a, token_b)
                    .call()
                    .await
                    .unwrap(),
            );
            pairs.push(Pair {
                token_a: pool.reserve_a.token,
                token_b: pool.reserve_b.token,
                contract: pair.clone(),
                pool: pool.to_owned(),
            });
            if pool.reserve_a.token == "WETH" {
                wait_for(
                    &web3,
                    weth.transfer(pair.address(), pool.reserve_a.amount)
                        .from(primary_account(&web3).await)
                        .send(),
                )
                .await
                .unwrap();
                wait_for(
                    &web3,
                    weth.transfer(settlement.address(), pool.reserve_a.amount)
                        .from(primary_account(&web3).await)
                        .send(),
                )
                .await
                .unwrap();
            } else {
                wait_for(
                    &web3,
                    tokens
                        .get(pool.reserve_a.token)
                        .unwrap()
                        .mint(pair.address(), pool.reserve_a.amount)
                        .from(trader_account.clone())
                        .send(),
                )
                .await
                .unwrap();
                wait_for(
                    &web3,
                    tokens
                        .get(pool.reserve_a.token)
                        .unwrap()
                        .mint(settlement.address(), pool.reserve_a.amount)
                        .from(trader_account.clone())
                        .send(),
                )
                .await
                .unwrap();
            }
            if pool.reserve_b.token == "WETH" {
                wait_for(
                    &web3,
                    weth.transfer(pair.address(), pool.reserve_b.amount)
                        .from(primary_account(&web3).await)
                        .send(),
                )
                .await
                .unwrap();
                wait_for(
                    &web3,
                    weth.transfer(settlement.address(), pool.reserve_b.amount)
                        .from(primary_account(&web3).await)
                        .send(),
                )
                .await
                .unwrap();
            } else {
                wait_for(
                    &web3,
                    tokens
                        .get(pool.reserve_b.token)
                        .unwrap()
                        .mint(pair.address(), pool.reserve_b.amount)
                        .from(trader_account.clone())
                        .send(),
                )
                .await
                .unwrap();
                wait_for(
                    &web3,
                    tokens
                        .get(pool.reserve_b.token)
                        .unwrap()
                        .mint(settlement.address(), pool.reserve_b.amount)
                        .from(trader_account.clone())
                        .send(),
                )
                .await
                .unwrap();
            }
            wait_for(
                &web3,
                pair.mint(
                    "0x8270bA71b28CF60859B547A2346aCDE824D6ed40"
                        .parse()
                        .unwrap(),
                )
                .from(trader_account.clone())
                .send(),
            )
            .await
            .unwrap();
        }

        // UniswapV2Pair._update, which is called by both mint() and swap(), will check
        // the block.timestamp and decide what to do based on it. If the block.timestamp
        // has changed since the last _update call, a conditional block will be
        // executed, which affects the gas used. The mint call above will result in the
        // first call to _update, and the onchain settlement will be the second.
        //
        // This timeout ensures that when the settlement is executed at least one UNIX
        // second has passed, so that conditional block always gets executed and the
        // gas usage is deterministic.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

        Self {
            trader_address: config.trader_address,
            trader_secret_key: config.trader_secret_key,
            solver_address: config.solver_address,
            solver_secret_key: config.solver_secret_key,
            tokens,
            settlement,
            domain_separator,
            weth,
            web3,
            geth,
            pairs,
            bogus_calldata: config.bogus_calldata,
        }
    }

    /// Set up the blockchain context and return the interactions needed to
    /// fulfill the orders.
    pub async fn fulfill(&self, orders: &[Order]) -> Vec<Fulfillment> {
        let mut fulfillments = Vec::new();
        for order in orders {
            // Find the pair to use for this order and calculate the buy and sell amounts.
            let sell_token = contracts::ERC20::at(&self.web3, self.get_token(order.sell_token));
            let buy_token = contracts::ERC20::at(&self.web3, self.get_token(order.buy_token));
            let pair = self
                .pairs
                .iter()
                .find(|pair| {
                    (pair.token_a, pair.token_b) == (order.sell_token, order.buy_token)
                        || (pair.token_b, pair.token_a) == (order.sell_token, order.buy_token)
                })
                .expect("could not find uniswap pair for order");
            let (sell_amount, buy_amount) = match order.side {
                order::Side::Buy => (
                    pair.pool.out(Asset {
                        token: order.buy_token,
                        amount: match order.solver_sell_diff {
                            super::Diff::Add(diff) => order.amount + diff,
                            super::Diff::Sub(diff) => order.amount - diff,
                        },
                    }),
                    order.amount,
                ),
                order::Side::Sell => (
                    order.amount,
                    pair.pool.out(Asset {
                        token: order.sell_token,
                        amount: match order.solver_buy_diff {
                            super::Diff::Add(diff) => order.amount + diff,
                            super::Diff::Sub(diff) => order.amount - diff,
                        },
                    }),
                ),
            };

            // Fund the trader account with tokens needed for the solution.
            let trader_account = ethcontract::Account::Offline(
                ethcontract::PrivateKey::from_slice(self.trader_secret_key.as_ref()).unwrap(),
                None,
            );
            if order.sell_token == "WETH" {
                todo!("deposit trader funds into the weth contract, none of the tests do this yet")
            } else {
                wait_for(
                    &self.web3,
                    self.tokens
                        .get(order.sell_token)
                        .unwrap()
                        .mint(
                            self.trader_address,
                            eth::U256::from(2) * sell_amount + order.user_fee,
                        )
                        .from(trader_account.clone())
                        .send(),
                )
                .await
                .unwrap();
            }

            // Approve the tokens needed for the solution.
            let vault_relayer = self.settlement.vault_relayer().call().await.unwrap();
            wait_for(
                &self.web3,
                self.tokens
                    .get(order.sell_token)
                    .unwrap()
                    .approve(vault_relayer, ethcontract::U256::max_value())
                    .from(trader_account.clone())
                    .send(),
            )
            .await
            .unwrap();

            // Create the interactions fulfilling the order.
            let transfer_interaction = sell_token
                .transfer(pair.contract.address(), sell_amount)
                .tx
                .data
                .unwrap()
                .0;
            let (amount_0_out, amount_1_out) = if pair.token_a == order.sell_token {
                (0.into(), buy_amount)
            } else {
                (buy_amount, 0.into())
            };
            let swap_interaction = pair
                .contract
                .swap(
                    amount_0_out,
                    amount_1_out,
                    self.settlement.address(),
                    Default::default(),
                )
                .tx
                .data
                .unwrap()
                .0;
            fulfillments.push(Fulfillment {
                order: order.clone(),
                interactions: vec![
                    Interaction {
                        address: sell_token.address(),
                        calldata: if self.bogus_calldata {
                            vec![1, 2, 3, 4, 5]
                        } else {
                            transfer_interaction
                        },
                        inputs: Default::default(),
                        outputs: Default::default(),
                        internalize: false,
                    },
                    Interaction {
                        address: pair.contract.address(),
                        calldata: if self.bogus_calldata {
                            vec![10, 11, 12, 13, 14, 15, 63, 78]
                        } else {
                            swap_interaction
                        },
                        inputs: vec![eth::Asset {
                            token: sell_token.address().into(),
                            amount: sell_amount,
                        }],
                        outputs: vec![eth::Asset {
                            token: buy_token.address().into(),
                            amount: buy_amount,
                        }],
                        internalize: order.internalize,
                    },
                ],
                sell_amount,
                buy_amount,
            });
        }
        fulfillments
    }

    pub fn get_token(&self, token: &str) -> eth::H160 {
        if token == "WETH" {
            self.weth.address()
        } else {
            self.tokens.get(token).unwrap().address()
        }
    }
}

async fn primary_address(web3: &DynWeb3) -> ethcontract::H160 {
    web3.eth().accounts().await.unwrap()[0]
}

async fn primary_account(web3: &DynWeb3) -> ethcontract::Account {
    ethcontract::Account::Local(web3.eth().accounts().await.unwrap()[0], None)
}

/// An instance of geth managed by the `dev-geth` crate. When this type is
/// dropped, the geth instance gets shut down.
#[derive(Debug)]
pub struct Geth {
    port: String,
}

impl Geth {
    /// Setup a new geth instance.
    async fn new() -> Self {
        let http = reqwest::Client::new();
        let res = http
            .post(format!("http://localhost:{DEV_GETH_PORT}"))
            .send()
            .await
            .unwrap();
        let port = res.text().await.unwrap();
        Self { port }
    }

    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
}

// What we really want here is "AsyncDrop", which is an unsolved problem in the
// async ecosystem. As a workaround we create a new runtime so that we can block
// on the delete request. Spawning a task for this isn't enough because tokio
// runtimes when they exit drop background tasks, like when a #[tokio::test]
// function returns.
impl Drop for Geth {
    fn drop(&mut self) {
        let port = std::mem::take(&mut self.port);
        let task = async move {
            let client = reqwest::Client::new();
            client
                .delete(&format!("http://localhost:{DEV_GETH_PORT}/{port}"))
                .send()
                .await
                .unwrap();
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // block_on must be called in a new thread because tokio forbids nesting
        // runtimes.
        let handle = std::thread::spawn(move || runtime.block_on(task));
        handle.join().unwrap();
    }
}

/// Execute an asynchronous operation, then wait for the next block to be mined
/// before proceeding.
///
/// [Dev mode geth](https://geth.ethereum.org/docs/developers/dapp-developer/dev-mode)
/// mines blocks as soon as there's a pending transaction, but publishing a
/// transaction does not wait for the block to be mined before returning. This
/// introduces a subtle race condition, so it's necessary to
/// wait for transactions to be confirmed before proceeding with the test. When
/// switching from geth back to hardhat, this function can be removed.
pub async fn wait_for<T>(web3: &DynWeb3, fut: impl Future<Output = T>) -> T {
    let block = web3.eth().block_number().await.unwrap();
    let result = fut.await;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let next_block = web3.eth().block_number().await.unwrap();
            if next_block > block {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("timeout while waiting for next block to be mined");
    result
}

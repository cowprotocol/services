use {
    super::{Asset, Order, Partial, Score},
    crate::{
        domain::{
            competition::order,
            eth::{self, ContractAddress},
        },
        infra::time,
        tests::{self, boundary, setup::FeePolicy},
    },
    ethcontract::{dyns::DynWeb3, transport::DynTransport, Web3},
    futures::Future,
    secp256k1::SecretKey,
    serde_json::json,
    std::collections::HashMap,
};

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
    pub trader_address: ethcontract::H160,
    pub trader_secret_key: SecretKey,

    pub web3: Web3<DynTransport>,
    pub web3_url: String,
    pub tokens: HashMap<&'static str, contracts::ERC20Mintable>,
    pub weth: contracts::WETH9,
    pub settlement: contracts::GPv2Settlement,
    pub ethflow: Option<ContractAddress>,
    pub domain_separator: boundary::DomainSeparator,
    pub node: Node,
    pub pairs: Vec<Pair>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct Solution {
    pub fulfillments: Vec<Fulfillment>,
    pub score: Score,
}

#[derive(Debug, Clone)]
pub struct Fulfillment {
    pub quoted_order: QuotedOrder,
    pub interactions: Vec<Interaction>,
}

/// An order for which buy and sell amounts have been calculated.
#[derive(Debug, Clone)]
pub struct QuotedOrder {
    pub order: Order,
    pub buy: eth::U256,
    pub sell: eth::U256,
}

impl QuotedOrder {
    /// The buy amount with the surplus factor.
    pub fn buy_amount(&self) -> eth::U256 {
        match self.order.side {
            order::Side::Buy => self.buy,
            order::Side::Sell => {
                let fee = match &self.order.fee_policy {
                    FeePolicy::PriceImprovement {
                        factor: _,
                        max_volume_factor: _,
                        quote,
                    } => quote.network_fee,
                    _ => eth::U256::zero(),
                };
                (self.buy * (self.sell - fee) / self.sell) / self.order.surplus_factor
            }
        }
    }

    /// The sell amount with the surplus factor.
    pub fn sell_amount(&self) -> eth::U256 {
        match self.order.side {
            order::Side::Buy => {
                let fee = match &self.order.fee_policy {
                    FeePolicy::PriceImprovement {
                        factor: _,
                        max_volume_factor: _,
                        quote,
                    } => quote.network_fee,
                    _ => eth::U256::zero(),
                };
                (self.sell + fee) * self.order.surplus_factor
            }
            order::Side::Sell => self.sell,
        }
    }

    /// The UID of the order.
    pub fn order_uid(&self, blockchain: &Blockchain) -> tests::boundary::OrderUid {
        self.boundary(blockchain).uid()
    }

    /// The signature of the order.
    pub fn order_signature(&self, blockchain: &Blockchain) -> Vec<u8> {
        self.boundary(blockchain).signature()
    }

    fn boundary(&self, blockchain: &Blockchain) -> tests::boundary::Order {
        tests::boundary::Order {
            sell_token: blockchain.get_token(self.order.sell_token),
            buy_token: blockchain.get_token(self.order.buy_token),
            sell_amount: self.sell_amount(),
            buy_amount: self.buy_amount(),
            valid_to: u32::try_from(time::now().timestamp()).unwrap() + self.order.valid_for.0,
            user_fee: self.order.user_fee,
            side: self.order.side,
            secret_key: blockchain.trader_secret_key,
            domain_separator: blockchain.domain_separator,
            owner: blockchain.trader_address,
            partially_fillable: matches!(self.order.partial, Partial::Yes { .. }),
        }
    }
}

pub struct Config {
    pub pools: Vec<Pool>,
    pub trader_address: eth::H160,
    pub trader_secret_key: SecretKey,
    pub solvers: Vec<super::Solver>,
    pub settlement_address: Option<eth::H160>,
}

impl Blockchain {
    /// Start a local node and deploy the
    /// settlement contract, token contracts, and all supporting contracts
    /// for the settlement.
    pub async fn new(config: Config) -> Self {
        // TODO All these various deployments that are happening from the trader account
        // should be happening from the primary_account of the node, will do this
        // later

        let node = Node::new().await;
        let web3 = Web3::new(DynTransport::new(
            web3::transports::Http::new(&node.url()).expect("valid URL"),
        ));

        let trader_account = ethcontract::Account::Offline(
            ethcontract::PrivateKey::from_slice(config.trader_secret_key.as_ref()).unwrap(),
            None,
        );

        // Use the primary account to fund the trader and the solver with ETH.
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

        // Deploy WETH and wrap some funds in the primary account of the node.
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
        let mut settlement = wait_for(
            &web3,
            contracts::GPv2Settlement::builder(&web3, authenticator.address(), vault.address())
                .from(trader_account.clone())
                .deploy(),
        )
        .await
        .unwrap();
        if let Some(settlement_address) = config.settlement_address {
            let vault_relayer = settlement.vault_relayer().call().await.unwrap();
            let vault_relayer_code = {
                // replace the vault relayer code to allow the settlement
                // contract at a specific address.
                let mut code = web3.eth().code(vault_relayer, None).await.unwrap().0;
                for i in 0..code.len() - 20 {
                    let window = &mut code[i..][..20];
                    if window == settlement.address().0 {
                        window.copy_from_slice(&settlement_address.0);
                    }
                }
                code
            };
            let settlement_code = web3.eth().code(settlement.address(), None).await.unwrap().0;

            set_code(&web3, vault_relayer, &vault_relayer_code).await;
            set_code(&web3, settlement_address, &settlement_code).await;

            settlement = contracts::GPv2Settlement::at(&web3, settlement_address);
        }
        wait_for(
            &web3,
            authenticator
                .initialize_manager(config.trader_address)
                .from(trader_account.clone())
                .send(),
        )
        .await
        .unwrap();

        for config in config.solvers {
            wait_for(
                &web3,
                authenticator
                    .add_solver(config.address())
                    .from(trader_account.clone())
                    .send(),
            )
            .await
            .unwrap();
            wait_for(
                &web3,
                web3.eth()
                    .send_transaction(web3::types::TransactionRequest {
                        from: primary_address(&web3).await,
                        to: Some(config.address()),
                        value: Some(config.balance),
                        ..Default::default()
                    }),
            )
            .await
            .unwrap();
        }

        let domain_separator =
            boundary::DomainSeparator(settlement.domain_separator().call().await.unwrap().0);

        // Create (deploy) the tokens needed by the pools.
        let mut tokens = HashMap::new();
        for pool in config.pools.iter() {
            if pool.reserve_a.token != "WETH" && !tokens.contains_key(pool.reserve_a.token) {
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
            if pool.reserve_b.token != "WETH" && !tokens.contains_key(pool.reserve_b.token) {
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
            tokens,
            settlement,
            domain_separator,
            weth,
            ethflow: None,
            web3,
            web3_url: node.url(),
            node,
            pairs,
        }
    }

    pub fn find_pair(&self, order: &Order) -> &Pair {
        self.pairs
            .iter()
            .find(|pair| {
                (pair.token_a, pair.token_b)
                    == (
                        order.sell_token,
                        if order.buy_token == "ETH" {
                            "WETH"
                        } else {
                            order.buy_token
                        },
                    )
                    || (pair.token_b, pair.token_a)
                        == (
                            order.sell_token,
                            if order.buy_token == "ETH" {
                                "WETH"
                            } else {
                                order.buy_token
                            },
                        )
            })
            .expect("could not find uniswap pair for order")
    }

    /// Quote an order using a UniswapV2 pool. This determines the buy and sell
    /// amount of the order.
    pub async fn quote(&self, order: &Order) -> QuotedOrder {
        let pair = self.find_pair(order);
        let executed_sell = order.sell_amount;
        let executed_buy = pair.pool.out(Asset {
            amount: order.sell_amount,
            token: order.sell_token,
        });
        QuotedOrder {
            order: order.clone(),
            buy: executed_buy,
            sell: executed_sell,
        }
    }

    /// Set up the blockchain context and return the interactions needed to
    /// fulfill the orders.
    pub async fn fulfill(
        &self,
        orders: impl Iterator<Item = &Order>,
        solution: &super::Solution,
    ) -> Solution {
        let mut fulfillments = Vec::new();
        for order in orders {
            // Find the pair to use for this order and calculate the buy and sell amounts.
            let sell_token =
                contracts::ERC20::at(&self.web3, self.get_token_wrapped(order.sell_token));
            let buy_token =
                contracts::ERC20::at(&self.web3, self.get_token_wrapped(order.buy_token));
            let pair = self.find_pair(order);
            let quote = self.quote(order).await;

            // Fund the trader account with tokens needed for the solution.
            let trader_account = ethcontract::Account::Offline(
                ethcontract::PrivateKey::from_slice(self.trader_secret_key.as_ref()).unwrap(),
                None,
            );
            if order.sell_token == "WETH" {
                todo!("deposit trader funds into the weth contract, none of the tests do this yet")
            } else if order.funded {
                wait_for(
                    &self.web3,
                    self.tokens
                        .get(order.sell_token)
                        .unwrap()
                        .mint(
                            self.trader_address,
                            eth::U256::from(100000000000u64) * quote.sell + order.user_fee,
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
                .transfer(pair.contract.address(), quote.sell)
                .tx
                .data
                .unwrap()
                .0;
            let (amount_a_out, amount_b_out) = if pair.token_a == order.sell_token {
                (0.into(), quote.buy)
            } else {
                // Surplus fees stay in the contract.
                (quote.sell - quote.order.surplus_fee(), 0.into())
            };
            let (amount_0_out, amount_1_out) =
                if self.get_token(pair.token_a) < self.get_token(pair.token_b) {
                    (amount_a_out, amount_b_out)
                } else {
                    (amount_b_out, amount_a_out)
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
                quoted_order: quote.clone(),
                interactions: vec![
                    Interaction {
                        address: sell_token.address(),
                        calldata: match solution.calldata {
                            super::Calldata::Valid { additional_bytes } => transfer_interaction
                                .into_iter()
                                .chain(std::iter::repeat(0xab).take(additional_bytes))
                                .collect(),
                            super::Calldata::Invalid => vec![1, 2, 3, 4, 5],
                        },
                        inputs: Default::default(),
                        outputs: Default::default(),
                        internalize: false,
                    },
                    Interaction {
                        address: pair.contract.address(),
                        calldata: match solution.calldata {
                            super::Calldata::Valid { .. } => swap_interaction,
                            super::Calldata::Invalid => {
                                vec![10, 11, 12, 13, 14, 15, 63, 78]
                            }
                        },
                        inputs: vec![eth::Asset {
                            token: sell_token.address().into(),
                            // Surplus fees stay in the contract.
                            amount: (quote.sell - quote.order.surplus_fee()).into(),
                        }],
                        outputs: vec![eth::Asset {
                            token: buy_token.address().into(),
                            amount: quote.buy.into(),
                        }],
                        internalize: order.internalize,
                    },
                ],
            });
        }
        Solution {
            fulfillments,
            score: solution.score.clone(),
        }
    }

    /// Returns the address of the token with the given symbol.
    pub fn get_token(&self, token: &str) -> eth::H160 {
        match token {
            "WETH" => self.weth.address(),
            "ETH" => eth::ETH_TOKEN.into(),
            _ => self.tokens.get(token).unwrap().address(),
        }
    }

    /// Returns the address of the token with the given symbol. Wrap ETH into
    /// WETH.
    pub fn get_token_wrapped(&self, token: &str) -> eth::H160 {
        match token {
            "WETH" | "ETH" => self.weth.address(),
            _ => self.tokens.get(token).unwrap().address(),
        }
    }
}

async fn primary_address(web3: &DynWeb3) -> ethcontract::H160 {
    web3.eth().accounts().await.unwrap()[0]
}

async fn primary_account(web3: &DynWeb3) -> ethcontract::Account {
    ethcontract::Account::Local(web3.eth().accounts().await.unwrap()[0], None)
}

/// A blockchain node for development purposes. Dropping this type will
/// terminate the node.
pub struct Node {
    process: tokio::process::Child,
    url: String,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node").field("url", &self.url).finish()
    }
}

impl Node {
    /// Spawn a new node instance.
    async fn new() -> Self {
        use tokio::io::AsyncBufReadExt as _;

        // Allow using some custom logic to spawn `anvil` by setting `ANVIL_COMMAND`.
        // For example if you set up a command that spins up a docker container.
        let command = std::env::var("ANVIL_COMMAND").unwrap_or("anvil".to_string());

        let mut process = tokio::process::Command::new(command)
            .arg("--port")
            .arg("0") // use 0 to let `anvil` use any open port
            .arg("--balance")
            .arg("1000000")
            .arg("--gas-limit")
            .arg("30000000")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = process.stdout.take().unwrap();
        let (sender, receiver) = tokio::sync::oneshot::channel::<String>();

        tokio::task::spawn(async move {
            let mut sender = Some(sender);
            const NEEDLE: &str = "Listening on ";
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            while let Some(line) = reader.next_line().await.unwrap() {
                tracing::trace!(line);
                if let Some(addr) = line.strip_prefix(NEEDLE) {
                    match sender.take() {
                        Some(sender) => sender.send(format!("http://{addr}")).unwrap(),
                        None => tracing::error!(addr, "detected multiple anvil endpoints"),
                    }
                }
            }
        });

        let url = tokio::time::timeout(tokio::time::Duration::from_secs(1), receiver)
            .await
            .expect("finding anvil URL timed out")
            .unwrap();
        Self { process, url }
    }

    fn url(&self) -> String {
        self.url.clone()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        // This only sends SIGKILL to the process but does not wait for the process to
        // actually terminate. But since `anvil` is fairly well behaved that
        // should be good enough.
        if let Err(err) = self.process.start_kill() {
            tracing::error!("failed to kill anvil: {err:?}");
        }
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
    let block = web3.eth().block_number().await.unwrap().as_u64();
    let result = fut.await;
    wait_for_block(web3, block + 1).await;
    result
}

/// Waits for the block height to be at least the specified value.
pub async fn wait_for_block(web3: &DynWeb3, block: u64) {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let next_block = web3.eth().block_number().await.unwrap().as_u64();
            if next_block >= block {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("timeout while waiting for next block to be mined");
}

/// Sets code at a specific address for testing.
pub async fn set_code(web3: &DynWeb3, address: eth::H160, code: &[u8]) {
    use web3::Transport;

    web3.transport()
        .execute(
            "anvil_setCode",
            vec![json!(address), json!(format!("0x{}", hex::encode(code)))],
        )
        .await
        .unwrap();
}

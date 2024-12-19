use {
    super::{Asset, Order, Partial},
    crate::{
        domain::{
            competition::order,
            eth::{self, ContractAddress},
        },
        tests::{self, boundary, cases::EtherExt},
    },
    ethcontract::{dyns::DynWeb3, transport::DynTransport, PrivateKey, Web3},
    futures::Future,
    secp256k1::SecretKey,
    serde_json::json,
    std::collections::HashMap,
    web3::{signing::Key, Transport},
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

#[allow(dead_code)]
#[derive(Debug)]
pub struct Blockchain {
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
    pub fn out_given_in(&self, input: Asset) -> eth::U256 {
        let (input_reserve, output_reserve) = if input.token == self.reserve_a.token {
            (self.reserve_a.amount, self.reserve_b.amount)
        } else {
            (self.reserve_b.amount, self.reserve_a.amount)
        };
        output_reserve * input.amount * eth::U256::from(997)
            / (input_reserve * eth::U256::from(1000) + input.amount * eth::U256::from(997))
    }

    /// Use the Uniswap constant AMM formula to calculate the input amount
    /// based on the output.
    pub fn in_given_out(&self, output: Asset) -> eth::U256 {
        let (input_reserve, output_reserve) = if output.token == self.reserve_b.token {
            (self.reserve_a.amount, self.reserve_b.amount)
        } else {
            (self.reserve_b.amount, self.reserve_a.amount)
        };

        ((input_reserve * output.amount * eth::U256::from(1000))
            / ((output_reserve - output.amount) * eth::U256::from(997)))
            + 1
    }
}

#[derive(Debug, Clone)]
pub struct Solution {
    pub trades: Vec<Trade>,
}

#[derive(Debug, Clone)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Fulfillment),
}

impl Trade {
    pub fn from_fulfillment(fulfillment: Fulfillment) -> Self {
        Trade::Fulfillment(fulfillment)
    }

    pub fn from_jit(mut fulfillment: Fulfillment) -> Self {
        // The JIT orders do not have interactions in the tests for the time being
        fulfillment.interactions = vec![];
        Trade::Jit(fulfillment)
    }
}

#[derive(Debug, Clone)]
pub struct Fulfillment {
    pub quoted_order: QuotedOrder,
    pub execution: Execution,
    pub interactions: Vec<Interaction>,
}

/// An order for which buy and sell amounts have been calculated.
#[derive(Debug, Clone)]
pub struct QuotedOrder {
    pub order: Order,
    pub buy: eth::U256,
    pub sell: eth::U256,
}

/// An execution of a trade with buy and sell amounts
#[derive(Debug, Clone)]
pub struct Execution {
    pub buy: eth::U256,
    pub sell: eth::U256,
}

impl QuotedOrder {
    /// The buy amount with the surplus factor.
    pub fn buy_amount(&self) -> eth::U256 {
        match self.order.side {
            order::Side::Buy => self.buy,
            order::Side::Sell => self.buy / self.order.surplus_factor,
        }
    }

    /// The sell amount with the surplus factor.
    pub fn sell_amount(&self) -> eth::U256 {
        match self.order.side {
            order::Side::Buy => self.sell * self.order.surplus_factor,
            order::Side::Sell => self.sell,
        }
    }

    /// The UID of the order.
    pub fn order_uid(&self, blockchain: &Blockchain) -> tests::boundary::OrderUid {
        self.boundary(blockchain, blockchain.trader_secret_key)
            .uid()
    }

    /// The signature of the order.
    pub fn order_signature(&self, blockchain: &Blockchain) -> Vec<u8> {
        self.boundary(blockchain, blockchain.trader_secret_key)
            .signature()
    }

    /// The signature of the order with a specific private key
    pub fn order_signature_with_private_key(
        &self,
        blockchain: &Blockchain,
        private_key: &PrivateKey,
    ) -> Vec<u8> {
        self.boundary(blockchain, **private_key).signature()
    }

    fn boundary(&self, blockchain: &Blockchain, secret_key: SecretKey) -> tests::boundary::Order {
        tests::boundary::Order {
            sell_token: blockchain.get_token(self.order.sell_token),
            buy_token: blockchain.get_token(self.order.buy_token),
            sell_amount: self.sell_amount(),
            buy_amount: self.buy_amount(),
            valid_to: self.order.valid_to,
            receiver: self.order.receiver,
            user_fee: self.order.fee_amount,
            side: self.order.side,
            secret_key,
            domain_separator: blockchain.domain_separator,
            owner: (&secret_key).address(),
            partially_fillable: matches!(self.order.partial, Partial::Yes { .. }),
        }
    }
}

pub struct Config {
    pub pools: Vec<Pool>,
    // Main trader secret key (the account deploying the contracts)
    pub main_trader_secret_key: SecretKey,
    pub solvers: Vec<super::Solver>,
    pub settlement_address: Option<eth::H160>,
    pub rpc_args: Vec<String>,
}

impl Blockchain {
    /// Start a local node and deploy the
    /// settlement contract, token contracts, and all supporting contracts
    /// for the settlement.
    pub async fn new(config: Config) -> Self {
        // TODO All these various deployments that are happening from the trader account
        // should be happening from the primary_account of the node, will do this
        // later

        let node = Node::new(&config.rpc_args).await;
        let web3 = Web3::new(DynTransport::new(
            web3::transports::Http::new(&node.url()).expect("valid URL"),
        ));

        let main_trader_account = ethcontract::Account::Offline(
            ethcontract::PrivateKey::from_slice(config.main_trader_secret_key.as_ref()).unwrap(),
            None,
        );

        // Use the primary account to fund the trader, cow amm and the solver with ETH.
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
                    to: Some(main_trader_account.address()),
                    value: Some(balance / 5),
                    ..Default::default()
                }),
        )
        .await
        .unwrap();

        let weth = wait_for(
            &web3,
            contracts::WETH9::builder(&web3)
                .from(main_trader_account.clone())
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
            contracts::BalancerV2Authorizer::builder(&web3, main_trader_account.address())
                .from(main_trader_account.clone())
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
            .from(main_trader_account.clone())
            .deploy(),
        )
        .await
        .unwrap();
        let authenticator = wait_for(
            &web3,
            contracts::GPv2AllowListAuthentication::builder(&web3)
                .from(main_trader_account.clone())
                .deploy(),
        )
        .await
        .unwrap();
        let mut settlement = wait_for(
            &web3,
            contracts::GPv2Settlement::builder(&web3, authenticator.address(), vault.address())
                .from(main_trader_account.clone())
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
                .initialize_manager(main_trader_account.address())
                .from(main_trader_account.clone())
                .send(),
        )
        .await
        .unwrap();

        let mut trader_accounts = Vec::new();
        for config in config.solvers {
            wait_for(
                &web3,
                authenticator
                    .add_solver(config.address())
                    .from(main_trader_account.clone())
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

            if !config.balance.is_zero() {
                let trader_account = ethcontract::Account::Offline(
                    ethcontract::PrivateKey::from_slice(config.private_key.as_ref()).unwrap(),
                    None,
                );
                trader_accounts.push(trader_account);
            }
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
                        .from(main_trader_account.clone())
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
                        .from(main_trader_account.clone())
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
            contracts::UniswapV2Factory::builder(&web3, main_trader_account.address())
                .from(main_trader_account.clone())
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
                    .from(main_trader_account.clone())
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
                for trader_account in trader_accounts.iter() {
                    wait_for(
                        &web3,
                        weth.transfer(trader_account.address(), pool.reserve_a.amount)
                            .from(primary_account(&web3).await)
                            .send(),
                    )
                    .await
                    .unwrap();
                }
            } else {
                for trader_account in trader_accounts.iter() {
                    let vault_relayer = settlement.vault_relayer().call().await.unwrap();
                    wait_for(
                        &web3,
                        tokens
                            .get(pool.reserve_a.token)
                            .unwrap()
                            .approve(vault_relayer, ethcontract::U256::max_value())
                            .from(trader_account.clone())
                            .send(),
                    )
                    .await
                    .unwrap();
                }

                wait_for(
                    &web3,
                    tokens
                        .get(pool.reserve_a.token)
                        .unwrap()
                        .mint(pair.address(), pool.reserve_a.amount)
                        .from(main_trader_account.clone())
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
                        .from(main_trader_account.clone())
                        .send(),
                )
                .await
                .unwrap();

                for trader_account in trader_accounts.iter() {
                    wait_for(
                        &web3,
                        tokens
                            .get(pool.reserve_a.token)
                            .unwrap()
                            .mint(trader_account.address(), pool.reserve_a.amount)
                            .from(main_trader_account.clone())
                            .send(),
                    )
                    .await
                    .unwrap();
                }
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
                for trader_account in trader_accounts.iter() {
                    wait_for(
                        &web3,
                        weth.transfer(trader_account.address(), pool.reserve_b.amount)
                            .from(primary_account(&web3).await)
                            .send(),
                    )
                    .await
                    .unwrap();
                }
            } else {
                for trader_account in trader_accounts.iter() {
                    let vault_relayer = settlement.vault_relayer().call().await.unwrap();
                    wait_for(
                        &web3,
                        tokens
                            .get(pool.reserve_b.token)
                            .unwrap()
                            .approve(vault_relayer, ethcontract::U256::max_value())
                            .from(trader_account.clone())
                            .send(),
                    )
                    .await
                    .unwrap();
                }

                wait_for(
                    &web3,
                    tokens
                        .get(pool.reserve_b.token)
                        .unwrap()
                        .mint(pair.address(), pool.reserve_b.amount)
                        .from(main_trader_account.clone())
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
                        .from(main_trader_account.clone())
                        .send(),
                )
                .await
                .unwrap();
                for trader_account in trader_accounts.iter() {
                    wait_for(
                        &web3,
                        tokens
                            .get(pool.reserve_b.token)
                            .unwrap()
                            .mint(trader_account.address(), pool.reserve_b.amount)
                            .from(main_trader_account.clone())
                            .send(),
                    )
                    .await
                    .unwrap();
                }
            }
            wait_for(
                &web3,
                pair.mint(
                    "0x8270bA71b28CF60859B547A2346aCDE824D6ed40"
                        .parse()
                        .unwrap(),
                )
                .from(main_trader_account.clone())
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
            trader_secret_key: config.main_trader_secret_key,
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

    /// Quote an order using a UniswapV2 pool unless it already has concrete
    /// amounts. This determines the buy and sell amount of the
    /// order in the auction.
    pub fn quote(&self, order: &Order) -> QuotedOrder {
        let executed_sell = order.sell_amount;
        let executed_buy = order.buy_amount.unwrap_or(self.execution(order).buy);
        QuotedOrder {
            order: order.clone(),
            buy: executed_buy,
            sell: executed_sell,
        }
    }

    /// Compute the execution of an order given the available liquidity
    pub fn execution(&self, order: &Order) -> Execution {
        let pair = self.find_pair(order);
        match order.side {
            order::Side::Buy => {
                // For buy order with explicitly specified amounts, use the buy amount,
                // otherwise assume the full sell amount to compute the execution
                let executed = order.executed.or(order.buy_amount);
                match executed {
                    Some(executed) => Execution {
                        buy: executed,
                        sell: pair.pool.in_given_out(Asset {
                            amount: executed,
                            token: order.buy_token,
                        }),
                    },
                    None => Execution {
                        buy: pair.pool.out_given_in(Asset {
                            amount: order.sell_amount,
                            token: order.sell_token,
                        }),
                        sell: order.sell_amount,
                    },
                }
            }
            order::Side::Sell => {
                let executed = order
                    .executed
                    .map(|amount| amount + order.surplus_fee())
                    .unwrap_or(order.sell_amount);
                Execution {
                    buy: pair.pool.out_given_in(Asset {
                        amount: executed,
                        token: order.sell_token,
                    }),
                    sell: executed,
                }
            }
        }
    }

    /// Set up the blockchain context and return the interactions needed to
    /// fulfill the orders.
    pub async fn fulfill(
        &self,
        orders: impl Iterator<Item = &Order>,
        solution: &super::Solution,
    ) -> Vec<Fulfillment> {
        let mut fulfillments = Vec::new();
        for order in orders {
            // Find the pair to use for this order and calculate the buy and sell amounts.
            let sell_token =
                contracts::ERC20::at(&self.web3, self.get_token_wrapped(order.sell_token));
            let buy_token =
                contracts::ERC20::at(&self.web3, self.get_token_wrapped(order.buy_token));
            let pair = self.find_pair(order);
            let execution = self.execution(order);

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
                            trader_account.address(),
                            "1e-7".ether().into_wei() * execution.sell,
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
                .transfer(pair.contract.address(), execution.sell)
                .tx
                .data
                .unwrap()
                .0;
            let (amount_a_out, amount_b_out) = if pair.token_a == order.sell_token {
                (0.into(), execution.buy)
            } else {
                // Surplus fees stay in the contract.
                (execution.sell - order.surplus_fee(), 0.into())
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
                quoted_order: self.quote(order),
                execution: execution.clone(),
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
                            amount: (execution.sell - order.surplus_fee()).into(),
                        }],
                        outputs: vec![eth::Asset {
                            token: buy_token.address().into(),
                            amount: execution.buy.into(),
                        }],
                        internalize: order.internalize,
                    },
                ],
            });
        }
        fulfillments
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

    pub async fn set_auto_mining(&self, enabled: bool) {
        self.web3
            .transport()
            .execute("evm_setAutomine", vec![json!(enabled)])
            .await
            .unwrap();
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
    async fn new(extra_args: &[String]) -> Self {
        use tokio::io::AsyncBufReadExt as _;

        // Allow using some custom logic to spawn `anvil` by setting `ANVIL_COMMAND`.
        // For example if you set up a command that spins up a docker container.
        let command = std::env::var("ANVIL_COMMAND").unwrap_or("anvil".to_string());

        let mut process = tokio::process::Command::new(command)
            .arg("--port")
            .arg("0") // use 0 to let `anvil` use any open port
            .arg("--balance")
            .arg("1000000")
            .args(extra_args)
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

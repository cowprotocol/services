use {
    super::{Asset, Order, Partial},
    crate::{
        domain::{competition::order, eth},
        tests::{self, boundary, cases::EtherExt},
    },
    alloy::{
        primitives::{Address, U256},
        providers::{Provider, ext::AnvilApi},
        rpc::types::TransactionRequest,
        signers::local::{MnemonicBuilder, PrivateKeySigner},
        sol_types::SolCall,
    },
    contracts::{
        BalancerV2Authorizer, BalancerV2Vault, ERC20, ERC20Mintable, FlashLoanRouter,
        GPv2AllowListAuthentication::GPv2AllowListAuthentication,
        GPv2Settlement, WETH9,
        support::{Balances, Signatures},
    },
    ethrpc::{
        Web3,
        alloy::{CallBuilderExt, EvmProviderExt, ProviderExt},
    },
    futures::Future,
    solvers_dto::solution::Flashloan,
    std::collections::HashMap,
};
// TODO Possibly might be a good idea to use an enum for tokens instead of
// &'static str

#[derive(Debug)]
pub struct Pair {
    token_a: &'static str,
    token_b: &'static str,
    contract: contracts::IUniswapLikePair::Instance,
    pool: Pool,
}

#[derive(Debug)]
pub struct Blockchain {
    pub trader_secret_key: PrivateKeySigner,
    pub web3: Web3,
    pub web3_url: String,
    pub web3_ws_url: String,
    pub tokens: HashMap<&'static str, ERC20Mintable::Instance>,
    pub weth: WETH9::Instance,
    pub settlement: GPv2Settlement::Instance,
    pub balances: Balances::Instance,
    pub signatures: Signatures::Instance,
    pub flashloan_router: FlashLoanRouter::Instance,
    pub domain_separator: boundary::DomainSeparator,
    #[allow(
        dead_code,
        reason = "we need to keep the node alive to run the `Drop` implementation at the \
                  appropriate time"
    )]
    pub node: Node,
    pub pairs: Vec<Pair>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Interaction {
    pub address: eth::Address,
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
            + eth::U256::ONE
    }
}

#[derive(Debug, Clone)]
pub struct Solution {
    pub trades: Vec<Trade>,
    pub flashloans: HashMap<order::Uid, Flashloan>,
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
        self.boundary(blockchain, blockchain.trader_secret_key.clone())
            .uid()
    }

    /// The signature of the order.
    pub fn order_signature(&self, blockchain: &Blockchain) -> Vec<u8> {
        self.boundary(blockchain, blockchain.trader_secret_key.clone())
            .signature()
    }

    /// The signature of the order with a specific private key
    pub fn order_signature_with_private_key(
        &self,
        blockchain: &Blockchain,
        signer: PrivateKeySigner,
    ) -> Vec<u8> {
        self.boundary(blockchain, signer).signature()
    }

    fn boundary(
        &self,
        blockchain: &Blockchain,
        signer: PrivateKeySigner,
    ) -> tests::boundary::Order {
        let owner = signer.address();
        tests::boundary::Order {
            sell_token: blockchain.get_token(self.order.sell_token),
            buy_token: blockchain.get_token(self.order.buy_token),
            sell_amount: self.sell_amount(),
            buy_amount: self.buy_amount(),
            valid_to: self.order.valid_to,
            receiver: self.order.receiver,
            user_fee: self.order.fee_amount,
            side: self.order.side,
            secret_key: signer,
            domain_separator: blockchain.domain_separator,
            owner,
            partially_fillable: matches!(self.order.partial, Partial::Yes { .. }),
        }
    }
}

pub struct Config {
    pub pools: Vec<Pool>,
    // Main trader secret key (the account deploying the contracts)
    pub main_trader_secret_key: PrivateKeySigner,
    pub solvers: Vec<super::Solver>,
    pub settlement_address: Option<eth::Address>,
    pub balances_address: Option<eth::Address>,
    pub signatures_address: Option<eth::Address>,
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
        let web3 = Web3::new_from_url(&node.url());

        let main_trader_address = config.main_trader_secret_key.address();
        web3.wallet
            .register_signer(config.main_trader_secret_key.clone());
        // This account is equivalent to the first test account, but due to the wallet
        // initialization process and the fact that we launch anvil manually, we need to
        // add it ourselves.
        // It also must be added after the main_trader because otherwise this will be
        // used as the default signing account
        let primary_account = MnemonicBuilder::english()
            .phrase("test test test test test test test test test test test junk")
            .index(0)
            .unwrap()
            .build()
            .unwrap();
        let primary_address = primary_account.address();
        web3.wallet.register_signer(primary_account);

        // Use the primary account to fund the trader, cow amm and the solver with ETH.
        let balance = web3.provider.get_balance(primary_address).await.unwrap();
        wait_for(
            &web3,
            web3.provider.send_and_watch(
                TransactionRequest::default()
                    .from(primary_address)
                    .to(main_trader_address)
                    .value(balance / alloy::primitives::U256::from(5)),
            ),
        )
        .await
        .unwrap();

        let weth = contracts::WETH9::Instance::deploy_builder(web3.provider.clone())
            .from(main_trader_address)
            .deploy()
            .await
            .unwrap();
        let weth = WETH9::WETH9::new(weth, web3.provider.clone());
        wait_for(
            &web3,
            web3.provider.send_and_watch(
                TransactionRequest::default()
                    .from(primary_address)
                    .to(*weth.address())
                    .value(balance / alloy::primitives::U256::from(5)),
            ),
        )
        .await
        .unwrap();

        // Set up the settlement contract and related contracts.
        let vault_authorizer = BalancerV2Authorizer::Instance::deploy_builder(
            web3.provider.clone(),
            main_trader_address,
        )
        .from(main_trader_address)
        .deploy()
        .await
        .unwrap();
        let vault = BalancerV2Vault::Instance::deploy_builder(
            web3.provider.clone(),
            vault_authorizer,
            *weth.address(),
            alloy::primitives::U256::ZERO,
            alloy::primitives::U256::ZERO,
        )
        .from(main_trader_address)
        .deploy()
        .await
        .unwrap();
        let authenticator = GPv2AllowListAuthentication::deploy(web3.provider.clone())
            .await
            .unwrap();
        let mut settlement = GPv2Settlement::GPv2Settlement::deploy(
            web3.provider.clone(),
            *authenticator.address(),
            vault,
        )
        .await
        .unwrap();
        if let Some(settlement_address) = config.settlement_address {
            let vault_relayer = settlement.vaultRelayer().call().await.unwrap();
            let vault_relayer_code = {
                // replace the vault relayer code to allow the settlement
                // contract at a specific address.
                let mut code = web3
                    .provider
                    .get_code_at(vault_relayer)
                    .await
                    .unwrap()
                    .to_vec();

                for i in 0..code.len() - 20 {
                    let window = &mut code[i..][..20];
                    if window == settlement.address().as_slice() {
                        window.copy_from_slice(settlement_address.as_slice());
                    }
                }
                code
            };
            web3.provider
                .anvil_set_code(vault_relayer, vault_relayer_code.into())
                .await
                .unwrap();

            // Note: (settlement.address() == authenticator_address) != settlement_address
            let settlement_code = web3
                .provider
                .get_code_at(*settlement.address())
                .await
                .unwrap();
            web3.provider
                .anvil_set_code(settlement_address, settlement_code)
                .await
                .unwrap();

            settlement =
                GPv2Settlement::GPv2Settlement::new(settlement_address, web3.provider.clone());
        }

        let balances_address = match config.balances_address {
            Some(balances_address) => balances_address,
            None => Balances::Instance::deploy_builder(web3.provider.clone())
                .from(main_trader_address)
                .deploy()
                .await
                .unwrap(),
        };
        let balances = Balances::Instance::new(balances_address, web3.provider.clone());

        authenticator
            .initializeManager(main_trader_address)
            .from(main_trader_address)
            .send_and_watch()
            .await
            .unwrap();

        let signatures_address = if let Some(signatures_address) = config.signatures_address {
            signatures_address
        } else {
            Signatures::Instance::deploy_builder(web3.provider.clone())
                .from(main_trader_address)
                .deploy()
                .await
                .unwrap()
        };
        let signatures = Signatures::Instance::new(signatures_address, web3.provider.clone());

        let flashloan_router_address =
            FlashLoanRouter::Instance::deploy_builder(web3.provider.clone(), *settlement.address())
                .from(main_trader_address)
                .deploy()
                .await
                .unwrap();
        let flashloan_router =
            FlashLoanRouter::Instance::new(flashloan_router_address, web3.provider.clone());

        let mut trader_addresses: Vec<Address> = Vec::new();
        for config in config.solvers {
            authenticator
                .addSolver(config.address())
                .from(main_trader_address)
                .send_and_watch()
                .await
                .unwrap();
            wait_for(
                &web3,
                web3.provider.send_and_watch(
                    TransactionRequest::default()
                        .from(primary_address)
                        .to(config.address())
                        .value(config.balance),
                ),
            )
            .await
            .unwrap();

            if !config.balance.is_zero() {
                trader_addresses.push(config.signer.address());
                web3.wallet.register_signer(config.signer);
            }
        }

        let domain_separator =
            boundary::DomainSeparator(settlement.domainSeparator().call().await.unwrap().0);

        // Create (deploy) the tokens needed by the pools.
        let mut tokens = HashMap::new();
        for pool in config.pools.iter() {
            if pool.reserve_a.token != "WETH" && !tokens.contains_key(pool.reserve_a.token) {
                let token = ERC20Mintable::Instance::deploy(web3.provider.clone())
                    .await
                    .unwrap();
                tokens.insert(pool.reserve_a.token, token);
            }
            if pool.reserve_b.token != "WETH" && !tokens.contains_key(pool.reserve_b.token) {
                let token = ERC20Mintable::Instance::deploy(web3.provider.clone())
                    .await
                    .unwrap();
                tokens.insert(pool.reserve_b.token, token);
            }
        }
        // Create the uniswap factory.
        let contract_address = contracts::UniswapV2Factory::Instance::deploy_builder(
            web3.provider.clone(),
            main_trader_address,
        )
        .from(main_trader_address)
        .deploy()
        .await
        .unwrap();
        let uniswap_factory =
            contracts::UniswapV2Factory::Instance::new(contract_address, web3.provider.clone());
        // Create and fund a uniswap pair for each pool. Fund the settlement contract
        // with the same liquidity as the pool, to allow for internalized interactions.
        let mut pairs = Vec::new();
        for pool in config.pools {
            // Get token addresses.
            let token_a = if pool.reserve_a.token == "WETH" {
                *weth.address()
            } else {
                *tokens.get(pool.reserve_a.token).unwrap().address()
            };
            let token_b = if pool.reserve_b.token == "WETH" {
                *weth.address()
            } else {
                *tokens.get(pool.reserve_b.token).unwrap().address()
            };
            // Create the pair.
            uniswap_factory
                .createPair(token_a, token_b)
                .from(main_trader_address)
                .send_and_watch()
                .await
                .unwrap();
            // Fund the pair and the settlement contract.
            let pair = contracts::IUniswapLikePair::Instance::new(
                uniswap_factory
                    .getPair(token_a, token_b)
                    .call()
                    .await
                    .unwrap(),
                web3.provider.clone(),
            );
            pairs.push(Pair {
                token_a: pool.reserve_a.token,
                token_b: pool.reserve_b.token,
                contract: pair.clone(),
                pool: pool.to_owned(),
            });
            if pool.reserve_a.token == "WETH" {
                weth.transfer(*pair.address(), pool.reserve_a.amount)
                    .from(primary_address)
                    .send_and_watch()
                    .await
                    .unwrap();
                weth.transfer(*settlement.address(), pool.reserve_a.amount)
                    .from(primary_address)
                    .send_and_watch()
                    .await
                    .unwrap();
                for trader_address in trader_addresses.iter().copied() {
                    weth.transfer(trader_address, pool.reserve_a.amount)
                        .from(primary_address)
                        .send_and_watch()
                        .await
                        .unwrap();
                }
            } else {
                for trader_address in trader_addresses.iter().copied() {
                    let vault_relayer = settlement.vaultRelayer().call().await.unwrap();

                    tokens
                        .get(pool.reserve_a.token)
                        .unwrap()
                        .approve(vault_relayer, U256::MAX)
                        .from(trader_address)
                        .send_and_watch()
                        .await
                        .unwrap();
                }

                tokens
                    .get(pool.reserve_a.token)
                    .unwrap()
                    .mint(*pair.address(), pool.reserve_a.amount)
                    .from(main_trader_address)
                    .send_and_watch()
                    .await
                    .unwrap();

                tokens
                    .get(pool.reserve_a.token)
                    .unwrap()
                    .mint(*settlement.address(), pool.reserve_a.amount)
                    .from(main_trader_address)
                    .send_and_watch()
                    .await
                    .unwrap();

                for trader_address in trader_addresses.iter().copied() {
                    tokens
                        .get(pool.reserve_a.token)
                        .unwrap()
                        .mint(trader_address, pool.reserve_a.amount)
                        .from(main_trader_address)
                        .send_and_watch()
                        .await
                        .unwrap();
                }
            }
            if pool.reserve_b.token == "WETH" {
                weth.transfer(*pair.address(), pool.reserve_b.amount)
                    .from(primary_address)
                    .send_and_watch()
                    .await
                    .unwrap();
                weth.transfer(*settlement.address(), pool.reserve_b.amount)
                    .from(primary_address)
                    .send_and_watch()
                    .await
                    .unwrap();
                for trader_address in trader_addresses.iter().copied() {
                    weth.transfer(trader_address, pool.reserve_b.amount)
                        .from(primary_address)
                        .send_and_watch()
                        .await
                        .unwrap();
                }
            } else {
                for trader_address in trader_addresses.iter().copied() {
                    let vault_relayer = settlement.vaultRelayer().call().await.unwrap();

                    tokens
                        .get(pool.reserve_b.token)
                        .unwrap()
                        .approve(vault_relayer, U256::MAX)
                        .from(trader_address)
                        .send_and_watch()
                        .await
                        .unwrap();
                }

                tokens
                    .get(pool.reserve_b.token)
                    .unwrap()
                    .mint(*pair.address(), pool.reserve_b.amount)
                    .from(main_trader_address)
                    .send_and_watch()
                    .await
                    .unwrap();

                tokens
                    .get(pool.reserve_b.token)
                    .unwrap()
                    .mint(*settlement.address(), pool.reserve_b.amount)
                    .from(main_trader_address)
                    .send_and_watch()
                    .await
                    .unwrap();
                for trader_address in trader_addresses.iter().copied() {
                    tokens
                        .get(pool.reserve_b.token)
                        .unwrap()
                        .mint(trader_address, pool.reserve_b.amount)
                        .from(main_trader_address)
                        .send_and_watch()
                        .await
                        .unwrap();
                }
            }
            pair.mint(::alloy::primitives::address!(
                "0x8270bA71b28CF60859B547A2346aCDE824D6ed40"
            ))
            .from(main_trader_address)
            .send_and_watch()
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
            balances,
            signatures,
            domain_separator,
            weth,
            web3,
            web3_url: node.url(),
            web3_ws_url: node.ws_url(),
            node,
            pairs,
            flashloan_router,
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
            let sell_token = ERC20::Instance::new(
                self.get_token_wrapped(order.sell_token),
                self.web3.provider.clone(),
            );
            let buy_token = ERC20::Instance::new(
                self.get_token_wrapped(order.buy_token),
                self.web3.provider.clone(),
            );
            let pair = self.find_pair(order);
            let execution = self.execution(order);

            // Register the trader account as a signer
            let trader_address = self.trader_secret_key.address();
            self.web3
                .wallet
                .register_signer(self.trader_secret_key.clone());

            // Fund the trader account with tokens needed for the solution.
            if order.sell_token == "WETH" {
                todo!("deposit trader funds into the weth contract, none of the tests do this yet")
            } else if order.funded {
                self.tokens
                    .get(order.sell_token)
                    .unwrap()
                    .mint(trader_address, "1e-7".ether().into_wei() * execution.sell)
                    .from(trader_address)
                    .send_and_watch()
                    .await
                    .unwrap();
            }

            // Approve the tokens needed for the solution.
            let vault_relayer = self.settlement.vaultRelayer().call().await.unwrap();

            self.tokens
                .get(order.sell_token)
                .unwrap()
                .approve(vault_relayer, U256::MAX)
                .from(trader_address)
                .send_and_watch()
                .await
                .unwrap();

            // Create the interactions fulfilling the order.
            let transfer_interaction = ERC20::ERC20::transferCall {
                recipient: *pair.contract.address(),
                amount: execution.sell,
            }
            .abi_encode();
            let (amount_a_out, amount_b_out) = if pair.token_a == order.sell_token {
                (eth::U256::ZERO, execution.buy)
            } else {
                // Surplus fees stay in the contract.
                (execution.sell - order.surplus_fee(), eth::U256::ZERO)
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
                    *self.settlement.address(),
                    Default::default(),
                )
                .calldata()
                .to_vec();
            fulfillments.push(Fulfillment {
                quoted_order: self.quote(order),
                execution: execution.clone(),
                interactions: vec![
                    Interaction {
                        address: *sell_token.address(),
                        calldata: match solution.calldata {
                            super::Calldata::Valid { additional_bytes } => transfer_interaction
                                .into_iter()
                                .chain(std::iter::repeat_n(0xab, additional_bytes))
                                .collect(),
                            super::Calldata::Invalid => vec![1, 2, 3, 4, 5],
                        },
                        inputs: Default::default(),
                        outputs: Default::default(),
                        internalize: false,
                    },
                    Interaction {
                        address: *pair.contract.address(),
                        calldata: match solution.calldata {
                            super::Calldata::Valid { .. } => swap_interaction,
                            super::Calldata::Invalid => {
                                vec![10, 11, 12, 13, 14, 15, 63, 78]
                            }
                        },
                        inputs: vec![eth::Asset {
                            token: (*sell_token.address()).into(),
                            // Surplus fees stay in the contract.
                            amount: (execution.sell - order.surplus_fee()).into(),
                        }],
                        outputs: vec![eth::Asset {
                            token: (*buy_token.address()).into(),
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
    pub fn get_token(&self, token: &str) -> Address {
        match token {
            "WETH" => *self.weth.address(),
            "ETH" => eth::ETH_TOKEN.0.0,
            _ => *self.tokens.get(token).unwrap().address(),
        }
    }

    /// Returns the address of the token with the given symbol. Wrap ETH into
    /// WETH.
    pub fn get_token_wrapped(&self, token: &str) -> Address {
        match token {
            "WETH" | "ETH" => *self.weth.address(),
            _ => *self.tokens.get(token).unwrap().address(),
        }
    }

    pub async fn set_auto_mining(&self, enabled: bool) {
        self.web3.provider.evm_set_automine(enabled).await.unwrap();
    }
}

/// A blockchain node for development purposes. Dropping this type will
/// terminate the node.
pub struct Node {
    process: tokio::process::Child,
    url: String,
    ws_url: String,
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

        let ws_url = url.replace("http://", "ws://");

        Self {
            process,
            url,
            ws_url,
        }
    }

    fn url(&self) -> String {
        self.url.clone()
    }

    fn ws_url(&self) -> String {
        self.ws_url.clone()
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
pub async fn wait_for<T>(web3: &Web3, fut: impl Future<Output = T>) -> T {
    let block = web3.provider.get_block_number().await.unwrap();
    let result = fut.await;
    wait_for_block(web3, block + 1).await;
    result
}

/// Waits for the block height to be at least the specified value.
pub async fn wait_for_block(web3: &Web3, block: u64) {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let next_block = web3.provider.get_block_number().await.unwrap();
            if next_block >= block {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("timeout while waiting for next block to be mined");
}

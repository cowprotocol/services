use {
    chrono::{DateTime, NaiveDateTime, Utc},
    contracts::{IZeroEx, ERC20},
    driver::domain::eth::H160,
    e2e::{
        api::zeroex::ZeroExApi,
        nodes::forked_node::ForkedNodeApi,
        setup::{
            colocation::{self, SolverEngine},
            run_forked_test_with_block_number,
            to_wei,
            to_wei_with_exp,
            wait_for_condition,
            OnchainComponents,
            Services,
            TestAccount,
            TIMEOUT,
        },
        tx,
    },
    ethcontract::{prelude::U256, private::lazy_static, H256},
    ethrpc::Web3,
    hex_literal::hex,
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
        DomainSeparator,
    },
    secp256k1::SecretKey,
    shared::zeroex_api::{
        Order,
        OrderMetadata,
        OrderRecord,
        OrdersQuery,
        ZeroExResponseError,
        ZeroExSignature,
    },
    std::{str::FromStr, sync::Arc},
    web3::{
        ethabi::{encode, Token},
        signing::{self, SecretKeyRef},
    },
};

/// The block number from which we will fetch state for the forked tests.
pub const FORK_BLOCK: u64 = 18477910;
pub const USDT_WHALE: H160 = H160(hex!("F977814e90dA44bFA03b6295A0616a897441aceC"));

#[tokio::test]
#[ignore]
async fn forked_node_zero_ex_liquidity_mainnet() {
    run_forked_test_with_block_number(
        zero_ex_liquidity,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK,
    )
    .await
}

async fn zero_ex_liquidity(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    let [trader, zeroex_maker] = onchain.make_accounts(to_wei(1)).await;
    forked_node_api.set_mocked_settle().await.unwrap();

    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_usdt = ERC20::at(
        &web3,
        "0xdac17f958d2ee523a2206206994597c13d831ec7"
            .parse()
            .unwrap(),
    );

    let zeroex = IZeroEx::deployed(&web3).await.unwrap();

    let amount = 500;

    // Give trader some USDC
    let usdc_whale = forked_node_api.impersonate(&USDT_WHALE).await.unwrap();
    tx!(
        usdc_whale,
        token_usdc.transfer(trader.address(), to_wei_with_exp(amount, 6))
    );

    // Give trader some USDT
    let usdt_whale = forked_node_api.impersonate(&USDT_WHALE).await.unwrap();
    tx!(
        usdt_whale,
        token_usdt.transfer(zeroex_maker.address(), to_wei_with_exp(amount * 3, 6))
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_usdc.approve(onchain.contracts().allowance, to_wei_with_exp(amount, 6))
    );
    tx!(
        zeroex_maker.account(),
        token_usdt.approve(zeroex.address(), to_wei_with_exp(amount * 3, 6))
    );

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: to_wei_with_exp(amount, 6),
        buy_token: token_usdt.address(),
        buy_amount: to_wei_with_exp(amount, 6),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let zeroex_api_port = {
        let order = order.clone();
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let weth_addr = onchain.contracts().weth.address();
        let gpv2_addr = onchain.contracts().gp_settlement.address();
        let zeroex_addr = zeroex.address();
        let orders_handler = Arc::new(Box::new(move |query: &OrdersQuery| {
            orders_query_handler(
                query,
                order.clone(),
                zeroex_maker.clone(),
                zeroex_addr,
                gpv2_addr,
                chain_id,
                weth_addr,
            )
        }));

        ZeroExApi::builder()
            .with_orders_handler(orders_handler)
            .build()
            .run()
            .await
    };

    // Place Orders
    let services = Services::new(onchain.contracts()).await;
    let solver_endpoint =
        colocation::start_baseline_solver(onchain.contracts().weth.address()).await;
    colocation::start_driver_with_zeroex_liquidity(
        onchain.contracts(),
        vec![SolverEngine {
            name: "test_solver".into(),
            account: solver,
            endpoint: solver_endpoint,
        }],
        zeroex_api_port,
    );
    services.start_autopilot(
        None,
        vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
            "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        ],
    );
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");
    let sell_token_balance_before = token_usdc
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    let buy_token_balance_before = token_usdt
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let sell_token_balance_after = token_usdc
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    let buy_token_balance_after = token_usdt
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    assert!(sell_token_balance_before > sell_token_balance_after);
    assert!(buy_token_balance_after >= buy_token_balance_before + to_wei_with_exp(amount, 6));
}

fn orders_query_handler(
    query: &OrdersQuery,
    order_creation: OrderCreation,
    zeroex_maker: TestAccount,
    zeroex_addr: H160,
    gpv2_addr: H160,
    chain_id: u64,
    weth_address: H160,
) -> Result<Vec<OrderRecord>, ZeroExResponseError> {
    if query.sender == Some(gpv2_addr) {
        let typed_order = Eip712TypedZeroExOrder {
            maker_token: order_creation.buy_token,
            taker_token: order_creation.sell_token,
            maker_amount: order_creation.buy_amount.as_u128() * 3,
            taker_amount: order_creation.sell_amount.as_u128() * 2,
            taker_token_fee_amount: 0,
            maker: zeroex_maker.address(),
            taker: gpv2_addr,
            sender: gpv2_addr,
            fee_recipient: zeroex_addr,
            pool: H256::default(),
            expiry: NaiveDateTime::MAX.timestamp() as u64,
            salt: U256::from(Utc::now().timestamp()),
        };
        let usdt_weth_order = Eip712TypedZeroExOrder {
            maker_token: order_creation.buy_token,
            taker_token: weth_address,
            maker_amount: order_creation.buy_amount.as_u128() * 3,
            taker_amount: order_creation.sell_amount.as_u128() * 2,
            taker_token_fee_amount: 0,
            maker: zeroex_maker.address(),
            taker: gpv2_addr,
            sender: gpv2_addr,
            fee_recipient: zeroex_addr,
            pool: H256::default(),
            expiry: NaiveDateTime::MAX.timestamp() as u64,
            salt: U256::from(Utc::now().timestamp()),
        };
        let usdc_weth_order = Eip712TypedZeroExOrder {
            maker_token: order_creation.buy_token,
            taker_token: weth_address,
            maker_amount: order_creation.buy_amount.as_u128() * 3,
            taker_amount: order_creation.sell_amount.as_u128() * 2,
            taker_token_fee_amount: 0,
            maker: zeroex_maker.address(),
            taker: gpv2_addr,
            sender: gpv2_addr,
            fee_recipient: zeroex_addr,
            pool: H256::default(),
            expiry: NaiveDateTime::MAX.timestamp() as u64,
            salt: U256::from(Utc::now().timestamp()),
        };
        Ok(vec![
            typed_order.to_order_record(chain_id, zeroex_addr, zeroex_maker.clone()),
            usdt_weth_order.to_order_record(chain_id, zeroex_addr, zeroex_maker.clone()),
            usdc_weth_order.to_order_record(chain_id, zeroex_addr, zeroex_maker),
        ])
    } else if query.sender
        == Some(H160::from_str("0x0000000000000000000000000000000000000000").unwrap())
    {
        Ok(vec![])
    } else {
        Err(ZeroExResponseError::ServerError(format!(
            "unexpected sender: {:?}",
            query.sender
        )))
    }
}

struct Eip712TypedZeroExOrder {
    maker_token: H160,
    taker_token: H160,
    maker_amount: u128,
    taker_amount: u128,
    taker_token_fee_amount: u128,
    maker: H160,
    taker: H160,
    sender: H160,
    fee_recipient: H160,
    pool: H256,
    expiry: u64,
    salt: U256,
}

impl Eip712TypedZeroExOrder {
    // See <https://github.com/0xProject/protocol/blob/%400x/contracts-zero-ex%400.49.0/contracts/zero-ex/contracts/src/features/libs/LibNativeOrder.sol#L112>
    const ZEROEX_LIMIT_ORDER_TYPEHASH: [u8; 32] =
        hex!("ce918627cb55462ddbb85e73de69a8b322f2bc88f4507c52fcad6d4c33c29d49");

    pub fn to_order_record(
        &self,
        chain_id: u64,
        verifying_contract: H160,
        signer: TestAccount,
    ) -> OrderRecord {
        OrderRecord {
            metadata: OrderMetadata {
                created_at: DateTime::<Utc>::MIN_UTC,
                order_hash: self.hash_struct().to_vec(),
                remaining_fillable_taker_amount: self.taker_amount,
            },
            order: Order {
                chain_id,
                expiry: NaiveDateTime::MAX.timestamp() as u64,
                fee_recipient: self.fee_recipient,
                maker: self.maker,
                maker_token: self.maker_token,
                maker_amount: self.maker_amount,
                pool: self.pool,
                salt: self.salt,
                sender: self.sender,
                taker: self.taker,
                taker_token: self.taker_token,
                taker_amount: self.taker_amount,
                taker_token_fee_amount: self.taker_token_fee_amount,
                verifying_contract,
                signature: self.sign(
                    &ZeroExDomainSeparator::new(chain_id, verifying_contract).to_domain_separator(),
                    self.hash_struct(),
                    signer,
                ),
            },
        }
    }

    fn sign(
        &self,
        domain_separator: &DomainSeparator,
        hash: [u8; 32],
        signer: TestAccount,
    ) -> ZeroExSignature {
        let signature = signer.sign_typed_data(domain_separator, &hash);
        ZeroExSignature {
            r: signature.r,
            s: signature.s,
            v: signature.v,
            // See <https://github.com/0xProject/protocol/blob/%400x/protocol-utils%4011.24.2/packages/protocol-utils/src/signature_utils.ts#L13>
            signature_type: 2,
        }
    }

    // See <https://github.com/0xProject/protocol/blob/%400x/contracts-zero-ex%400.49.0/contracts/zero-ex/contracts/src/features/libs/LibNativeOrder.sol#L166-L195>
    fn hash_struct(&self) -> [u8; 32] {
        let mut hash_data = [0u8; 416];
        hash_data[0..32].copy_from_slice(&Self::ZEROEX_LIMIT_ORDER_TYPEHASH);
        hash_data[44..64].copy_from_slice(self.maker_token.as_fixed_bytes());
        hash_data[76..96].copy_from_slice(self.taker_token.as_fixed_bytes());
        hash_data[112..128].copy_from_slice(&self.maker_amount.to_be_bytes());
        hash_data[144..160].copy_from_slice(&self.taker_amount.to_be_bytes());
        hash_data[176..192].copy_from_slice(&self.taker_token_fee_amount.to_be_bytes());
        hash_data[204..224].copy_from_slice(self.maker.as_fixed_bytes());
        hash_data[236..256].copy_from_slice(self.taker.as_fixed_bytes());
        hash_data[268..288].copy_from_slice(self.sender.as_fixed_bytes());
        hash_data[300..320].copy_from_slice(self.fee_recipient.as_fixed_bytes());
        hash_data[320..352].copy_from_slice(self.pool.as_fixed_bytes());
        hash_data[376..384].copy_from_slice(&self.expiry.to_be_bytes());
        self.salt.to_big_endian(&mut hash_data[384..416]);
        signing::keccak256(&hash_data)
    }
}

struct ZeroExDomainSeparator([u8; 32]);

impl ZeroExDomainSeparator {
    // See <https://github.com/0xProject/protocol/blob/%400x/contracts-zero-ex%400.49.0/contracts/zero-ex/contracts/src/fixins/FixinEIP712.sol>
    pub fn new(chain_id: u64, contract_addr: H160) -> Self {
        lazy_static! {
            /// The EIP-712 domain name used for computing the domain separator.
            static ref DOMAIN_NAME: [u8; 32] = signing::keccak256(b"ZeroEx");

            /// The EIP-712 domain version used for computing the domain separator.
            static ref DOMAIN_VERSION: [u8; 32] = signing::keccak256(b"1.0.0");

            /// The EIP-712 domain type used computing the domain separator.
            static ref DOMAIN_TYPE_HASH: [u8; 32] = signing::keccak256(
                b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
            );
        }
        let abi_encode_string = encode(&[
            Token::FixedBytes((*DOMAIN_TYPE_HASH).into()),
            Token::FixedBytes((*DOMAIN_NAME).into()),
            Token::FixedBytes((*DOMAIN_VERSION).into()),
            Token::Uint(chain_id.into()),
            Token::Address(contract_addr),
        ]);

        Self(signing::keccak256(abi_encode_string.as_slice()))
    }

    pub fn to_domain_separator(&self) -> DomainSeparator {
        DomainSeparator(self.0)
    }
}

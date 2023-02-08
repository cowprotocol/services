use {
    crate::tests::boundary,
    ethcontract::{transport::DynTransport, Web3},
    secp256k1::SecretKey,
};

#[derive(Debug)]
pub struct Uniswap {
    pub web3: Web3<DynTransport>,
    pub admin: ethcontract::H160,
    pub admin_secret_key: SecretKey,
    pub token_a: contracts::ERC20Mintable,
    pub token_b: contracts::ERC20Mintable,
    pub settlement: contracts::GPv2Settlement,
    pub domain_separator: boundary::DomainSeparator,
    pub weth: contracts::WETH9,
    pub token_a_in_amount: ethcontract::U256,
    pub token_b_out_amount: ethcontract::U256,
    pub user_fee: ethcontract::U256,
    /// Interactions needed for the solution.
    pub interactions: Vec<(ethcontract::H160, Vec<u8>)>,
    pub solver_address: ethcontract::H160,
    pub solver_secret_key: SecretKey,
    pub geth: super::Geth,
}

/// Set up a Uniswap V2 pair ready for the following swap:
///
///   /------------>(1. SELL 0.5 A for B)------------\
///   |                                              |
///   |                                              v
/// [USDT]<---(Uniswap Pair 1000 A / 600.000 B)--->[WETH]
pub async fn setup() -> Uniswap {
    let geth = super::geth().await;
    let web3 = super::web3(&geth.url());

    // Move ETH into the admin account.
    let admin = "d2525C68A663295BBE347B65C87c8e17De936a0a".parse().unwrap();
    let admin_secret_key = SecretKey::from_slice(
        &hex::decode("f9f831cee763ef826b8d45557f0f8677b27045e0e011bcd78571a40acc8a6cc3").unwrap(),
    )
    .unwrap();
    let admin_account = ethcontract::Account::Offline(
        ethcontract::PrivateKey::from_slice(admin_secret_key.as_ref()).unwrap(),
        None,
    );

    let solver_address = "0x72B92eE5f847fbb0d243047C263aCD40C34a63b9"
        .parse()
        .unwrap();
    let solver_secret_key = SecretKey::from_slice(
        &hex::decode("a131a35fb8f614b31611f4fe68b6fc538b0febd2f75cd68e1282d8fd45b63326").unwrap(),
    )
    .unwrap();

    let balance = web3
        .eth()
        .balance(super::primary_address(&web3).await, None)
        .await
        .unwrap();
    super::wait_for(
        &web3,
        web3.eth()
            .send_transaction(web3::types::TransactionRequest {
                from: super::primary_address(&web3).await,
                to: Some(admin),
                value: Some(balance / 3),
                ..Default::default()
            }),
    )
    .await
    .unwrap();
    super::wait_for(
        &web3,
        web3.eth()
            .send_transaction(web3::types::TransactionRequest {
                from: super::primary_address(&web3).await,
                to: Some(solver_address),
                value: Some(balance / 3),
                ..Default::default()
            }),
    )
    .await
    .unwrap();

    // Deploy contracts
    let weth = super::wait_for(
        &web3,
        contracts::WETH9::builder(&web3)
            .from(admin_account.clone())
            .deploy(),
    )
    .await
    .unwrap();
    let vault_authorizer = super::wait_for(
        &web3,
        contracts::BalancerV2Authorizer::builder(&web3, admin)
            .from(admin_account.clone())
            .deploy(),
    )
    .await
    .unwrap();
    let vault = super::wait_for(
        &web3,
        contracts::BalancerV2Vault::builder(
            &web3,
            vault_authorizer.address(),
            weth.address(),
            0.into(),
            0.into(),
        )
        .from(admin_account.clone())
        .deploy(),
    )
    .await
    .unwrap();
    let authenticator = super::wait_for(
        &web3,
        contracts::GPv2AllowListAuthentication::builder(&web3)
            .from(admin_account.clone())
            .deploy(),
    )
    .await
    .unwrap();
    let settlement = super::wait_for(
        &web3,
        contracts::GPv2Settlement::builder(&web3, authenticator.address(), vault.address())
            .from(admin_account.clone())
            .deploy(),
    )
    .await
    .unwrap();
    super::wait_for(
        &web3,
        authenticator
            .initialize_manager(admin)
            .from(admin_account.clone())
            .send(),
    )
    .await
    .unwrap();
    super::wait_for(
        &web3,
        authenticator
            .add_solver(solver_address)
            .from(admin_account.clone())
            .send(),
    )
    .await
    .unwrap();

    let domain_separator =
        boundary::DomainSeparator(settlement.domain_separator().call().await.unwrap().0);

    let token_a = super::wait_for(
        &web3,
        contracts::ERC20Mintable::builder(&web3)
            .from(admin_account.clone())
            .deploy(),
    )
    .await
    .unwrap();
    let token_b = super::wait_for(
        &web3,
        contracts::ERC20Mintable::builder(&web3)
            .from(admin_account.clone())
            .deploy(),
    )
    .await
    .unwrap();

    let uniswap_factory = super::wait_for(
        &web3,
        contracts::UniswapV2Factory::builder(&web3, admin)
            .from(admin_account.clone())
            .deploy(),
    )
    .await
    .unwrap();
    super::wait_for(
        &web3,
        uniswap_factory
            .create_pair(token_a.address(), token_b.address())
            .from(admin_account.clone())
            .send(),
    )
    .await
    .unwrap();
    let uniswap_pair = contracts::IUniswapLikePair::at(
        &web3,
        uniswap_factory
            .get_pair(token_a.address(), token_b.address())
            .call()
            .await
            .unwrap(),
    );

    let token_a_reserve = ethcontract::U256::from_dec_str("1000000000000000000000").unwrap();
    let token_b_reserve = ethcontract::U256::from_dec_str("600000000000").unwrap();

    super::wait_for(
        &web3,
        token_a
            .mint(uniswap_pair.address(), token_a_reserve)
            .from(admin_account.clone())
            .send(),
    )
    .await
    .unwrap();
    super::wait_for(
        &web3,
        token_b
            .mint(uniswap_pair.address(), token_b_reserve)
            .from(admin_account.clone())
            .send(),
    )
    .await
    .unwrap();
    super::wait_for(
        &web3,
        uniswap_pair
            .mint(
                "0x8270bA71b28CF60859B547A2346aCDE824D6ed40"
                    .parse()
                    .unwrap(),
            )
            .from(admin_account.clone())
            .send(),
    )
    .await
    .unwrap();
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

    let token_a_in_amount = ethcontract::U256::from_dec_str("500000000000000000").unwrap();
    // The out amount according to the constant AMM formula.
    let token_b_out_amount = ethcontract::U256::from_dec_str("298950972").unwrap();
    let user_fee = ethcontract::U256::from_dec_str("1000000000000000").unwrap();

    super::wait_for(
        &web3,
        token_a
            .mint(admin, token_a_in_amount + user_fee)
            .from(admin_account.clone())
            .send(),
    )
    .await
    .unwrap();

    let vault_relayer = settlement.vault_relayer().call().await.unwrap();
    super::wait_for(
        &web3,
        token_a
            .approve(vault_relayer, ethcontract::U256::max_value())
            .from(admin_account.clone())
            .send(),
    )
    .await
    .unwrap();

    let transfer_interaction = token_a
        .transfer(uniswap_pair.address(), token_a_in_amount)
        .tx
        .data
        .unwrap()
        .0;
    let (amount_0_out, amount_1_out) =
        if uniswap_pair.token_0().call().await.unwrap() == token_a.address() {
            (0.into(), token_b_out_amount)
        } else {
            (token_b_out_amount, 0.into())
        };
    let swap_interaction = uniswap_pair
        .swap(
            amount_0_out,
            amount_1_out,
            settlement.address(),
            Default::default(),
        )
        .tx
        .data
        .unwrap()
        .0;

    Uniswap {
        interactions: vec![
            (token_a.address(), transfer_interaction),
            (uniswap_pair.address(), swap_interaction),
        ],
        admin,
        token_a,
        token_b,
        settlement,
        domain_separator,
        token_a_in_amount,
        token_b_out_amount,
        user_fee,
        weth,
        web3,
        admin_secret_key,
        solver_address,
        solver_secret_key,
        geth,
    }
}

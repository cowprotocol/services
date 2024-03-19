use {
    e2e::{nodes::local_node::TestNodeApi, setup::*, tx, tx_value},
    ethcontract::{BlockId, H160, H256, U256},
    futures::{Stream, StreamExt},
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test_cancel_on_expiry).await;
}

async fn test_cancel_on_expiry(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let nonce = solver.nonce(&web3).await;
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(3))
    );
    tx_value!(
        trader.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver.clone()).await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.api::<TestNodeApi<_>>()
        .disable_automine()
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Placing order");
    let balance = token.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());
    let order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount: to_wei(2),
        fee_amount: 0.into(),
        buy_token: token.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();

    // Start tracking confirmed blocks so we can find the transaction later
    let block_stream = web3
        .eth_filter()
        .create_blocks_filter()
        .await
        .expect("must be able to create blocks filter")
        .stream(Duration::from_millis(50));

    // Wait for settlement tx to appear in txpool
    wait_for_condition(TIMEOUT, || async {
        get_pending_tx(solver.account().address(), &web3)
            .await
            .is_some()
    })
    .await
    .unwrap();

    // Restart mining, but with blocks that are too small to fit the settlement
    web3.api::<TestNodeApi<_>>()
        .set_block_gas_limit(100_000)
        .await
        .expect("Must be able to set block gas limit");
    web3.api::<TestNodeApi<_>>()
        .set_mining_interval(1)
        .await
        .expect("Must be able to set mining interval");

    // Wait for cancellation tx to appear
    wait_for_condition(TIMEOUT, || async { solver.nonce(&web3).await == nonce + 1 })
        .await
        .unwrap();

    // Check that it's actually a cancellation
    let tx = tokio::time::timeout(
        TIMEOUT,
        get_confirmed_transaction(solver.account().address(), &web3, block_stream),
    )
    .await
    .unwrap();
    assert_eq!(tx.to, Some(solver.account().address()))
}

async fn get_pending_tx(account: H160, web3: &Web3) -> Option<web3::types::Transaction> {
    let txpool = web3
        .txpool()
        .content()
        .await
        .expect("must be able to inspect mempool");
    txpool.pending.get(&account)?.values().next().cloned()
}

async fn get_confirmed_transaction(
    account: H160,
    web3: &Web3,
    block_stream: impl Stream<Item = Result<H256, web3::Error>>,
) -> web3::types::Transaction {
    let mut block_stream = Box::pin(block_stream);
    loop {
        let block_hash = block_stream.next().await.unwrap().unwrap();
        let block = web3
            .eth()
            .block_with_txs(BlockId::Hash(block_hash))
            .await
            .expect("must be able to get block by hash")
            .expect("block not found");
        for tx in block.transactions {
            if tx.from == Some(account) {
                return tx;
            }
        }
    }
}

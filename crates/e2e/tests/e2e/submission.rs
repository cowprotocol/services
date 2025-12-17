use {
    ::alloy::{
        primitives::{Address, B256, U256},
        providers::{Provider, ext::TxPoolApi},
        rpc::{
            client::PollerStream,
            types::{Transaction, TransactionReceipt},
        },
    },
    e2e::{nodes::local_node::TestNodeApi, setup::*},
    ethcontract::{BlockId, H160, H256},
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
    futures::{Stream, StreamExt},
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test_cancel_on_expiry).await;
}

async fn test_cancel_on_expiry(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let nonce = solver.nonce(&web3).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance.into_alloy(), 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.api::<TestNodeApi<_>>()
        .set_automine_enabled(false)
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Placing order");
    let balance = token.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(balance, U256::ZERO);
    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 2u64.eth(),
        buy_token: *token.address(),
        buy_amount: 1u64.eth(),
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
    onchain.mint_block().await;

    // Start tracking confirmed blocks so we can find the transaction later
    let stream = web3.alloy.watch_blocks().await.unwrap().into_stream();

    // Wait for settlement tx to appear in txpool
    wait_for_condition(TIMEOUT, || async {
        get_pending_tx(solver.address(), &web3).await.is_some()
    })
    .await
    .unwrap();

    // Restart mining, but with blocks that are too small to fit the settlement
    web3.alloy
        .raw_request::<(u64,), bool>("evm_setBlockGasLimit".into(), (100_000,))
        .await
        .expect("Must be able to set block gas limit");
    web3.alloy
        .raw_request::<(u64,), ()>("evm_setIntervalMining".into(), (1,))
        .await
        .expect("Must be able to set mining interval");

    // Wait for cancellation tx to appear
    wait_for_condition(TIMEOUT, || async { solver.nonce(&web3).await == nonce + 1 })
        .await
        .unwrap();

    // Check that it's actually a cancellation
    let tx = tokio::time::timeout(
        TIMEOUT,
        get_confirmed_transaction(solver.address(), &web3, stream),
    )
    .await
    .unwrap();
    assert_eq!(tx.to, Some(solver.address()))
}

async fn get_pending_tx(account: Address, web3: &Web3) -> Option<Transaction> {
    let txpool = web3
        .alloy
        .txpool_content()
        .await
        .expect("must be able to inspect mempool");
    txpool.pending.get(&account)?.values().next().cloned()
}

async fn get_confirmed_transaction(
    account: Address,
    web3: &Web3,
    block_hash_stream: PollerStream<Vec<B256>>,
) -> TransactionReceipt {
    let mut block_hash_stream = Box::pin(block_hash_stream);
    loop {
        let block_hashes = block_hash_stream.next().await.unwrap();
        for block_hash in block_hashes {
            let transaction_senders = web3
                .alloy
                .get_block_receipts(block_hash.into())
                .await
                .unwrap()
                .into_iter()
                .flatten();

            for tx in transaction_senders {
                if tx.from == account {
                    return tx;
                }
            }
        }
    }
}

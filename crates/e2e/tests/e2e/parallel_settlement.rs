use {
    ::alloy::{
        consensus::Transaction as _,
        eips::eip7702::Authorization,
        network::TransactionBuilder7702,
        primitives::{Address, U256},
        providers::{Provider, ext::TxPoolApi},
        rpc::types::TransactionRequest,
        signers::Signer,
        sol_types::SolCall,
    },
    contracts::alloy::CowSettlementForwarder::CowSettlementForwarder,
    e2e::setup::{colocation, *},
    ethrpc::{
        Web3,
        alloy::{CallBuilderExt, EvmProviderExt},
    },
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    std::time::Duration,
};

/// Tests that the driver can process two settlement requests concurrently,
/// resulting in both settlement transactions being pending in the mempool
/// simultaneously.
///
/// Bypasses the autopilot (which settles one solution per auction) and sends
/// two /solve + /settle requests directly to the driver.
///
/// Uses EIP-7702 delegation: a minimal forwarder contract is deployed and the
/// solver EOA delegates its code to it. Two submission accounts send settlement
/// txs through the solver EOA in parallel, each using their own nonce.
#[tokio::test]
#[ignore]
async fn local_node_parallel_settlement_submission() {
    run_test(test_parallel_settlement_submission).await;
}

async fn test_parallel_settlement_submission(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    // Two submission accounts for parallel settlement via EIP-7702.
    let [submitter_a, submitter_b] = onchain.make_accounts(10u64.eth()).await;

    // Deploy two independent token pairs so settlements don't conflict on the
    // same Uniswap pool when mined in the same block.
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader with WETH and approve the vault relayer.
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(5u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 5u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Deploy the settlement forwarder and set up EIP-7702 delegation on the
    // solver EOA. Then approve both submission accounts as callers.
    let forwarder_addr = deploy_forwarder(onchain.web3(), &submitter_a).await;
    setup_eip7702_delegation(onchain.web3(), &solver, &submitter_a, forwarder_addr).await;
    approve_submission_callers(
        onchain.web3(),
        &solver,
        &[submitter_a.address(), submitter_b.address()],
    )
    .await;

    // Start driver + baseline solver. Each /solve call is a separate auction
    // so solutions are independent regardless of merge_solutions.
    let mut solver_engine = colocation::start_baseline_solver(
        "test_solver".into(),
        solver.clone(),
        *onchain.contracts().weth.address(),
        vec![],
        1,
        false,
    )
    .await;
    solver_engine.submission_keys = vec![submitter_a.clone(), submitter_b.clone()];

    colocation::start_driver(
        onchain.contracts(),
        vec![solver_engine],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    // Wait for the driver to become available.
    let driver_url = "http://localhost:11088/test_solver";
    wait_for_condition(TIMEOUT, || async {
        reqwest::get(format!("{driver_url}/healthz")).await.is_ok()
    })
    .await
    .expect("driver did not start in time");

    let valid_to = model::time::now_in_epoch_seconds() + 300;
    let make_buy_order = |buy_token: Address| {
        OrderCreation {
            sell_token: *onchain.contracts().weth.address(),
            sell_amount: 2u64.eth(),
            buy_token,
            buy_amount: 1u64.eth(),
            valid_to,
            kind: OrderKind::Buy,
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader.signer,
        )
    };
    let order_a = make_buy_order(*token_a.address());
    let order_b = make_buy_order(*token_b.address());

    onchain.mint_block().await;

    let block_number = web3.provider.get_block_number().await.unwrap();
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    // Send /solve for each order as a separate auction. The driver caches
    // the resulting settlement for each, keyed by (auction_id, solution_id).
    let solution_id_a =
        solve_order(&http, driver_url, 1, &order_a, trader.address(), &onchain).await;
    let solution_id_b =
        solve_order(&http, driver_url, 2, &order_b, trader.address(), &onchain).await;

    // Disable automine so settlement txs stay pending in the mempool.
    web3.provider
        .evm_set_automine(false)
        .await
        .expect("Must be able to disable automine");

    // Send both /settle requests concurrently. With parallel submission, both
    // submission accounts send txs to the solver EOA simultaneously.
    let submission_deadline = block_number + 100;
    let settle_url = format!("{driver_url}/settle");
    let spawn_settle = |auction_id: &'static str, solution_id: u64| {
        let http = http.clone();
        let settle_url = settle_url.clone();
        tokio::spawn(async move {
            http.post(&settle_url)
                .json(&serde_json::json!({
                    "solutionId": solution_id,
                    "submissionDeadlineLatestBlock": submission_deadline,
                    "auctionId": auction_id
                }))
                .send()
                .await
        })
    };
    let settle_a = spawn_settle("1", solution_id_a);
    let settle_b = spawn_settle("2", solution_id_b);

    // Assert that TWO settlement txs are pending simultaneously.
    // In EIP-7702 mode, txs target the solver EOA (which delegates to the
    // forwarder) rather than the settlement contract directly.
    let solver_address = solver.address();
    let parallel_txs_observed = wait_for_condition(Duration::from_secs(15), || {
        let web3 = web3.clone();
        async move {
            let txpool = web3
                .provider
                .txpool_content()
                .await
                .expect("must be able to inspect mempool");
            let pending_settlements: usize = txpool
                .pending
                .values()
                .flat_map(|nonce_map| nonce_map.values())
                .filter(|tx| tx.inner.to() == Some(solver_address))
                .count();

            tracing::debug!(pending_settlements, "checking for parallel pending txs");
            pending_settlements >= 2
        }
    })
    .await;

    assert!(
        parallel_txs_observed.is_ok(),
        "Expected two pending settlement txs simultaneously targeting the solver EOA via EIP-7702 \
         delegation."
    );

    // Re-enable automine and verify both orders get settled.
    web3.provider
        .evm_set_automine(true)
        .await
        .expect("Must be able to enable automine");

    // Wait for the /settle HTTP calls to complete (they block until mined).
    assert_settle_success(settle_a.await, "1").await;
    assert_settle_success(settle_b.await, "2").await;

    for token in [&token_a, &token_b] {
        wait_for_condition(TIMEOUT, || async {
            let balance = token.balanceOf(trader.address()).call().await.unwrap();
            balance > U256::ZERO
        })
        .await
        .unwrap();
    }
}

/// Sends a /solve request to the driver for a single order and returns the
/// solution_id from the response.
async fn solve_order(
    http: &reqwest::Client,
    driver_url: &str,
    auction_id: i64,
    order: &OrderCreation,
    owner: Address,
    onchain: &OnchainComponents,
) -> u64 {
    let weth = onchain.contracts().weth.address();
    let uid = order
        .data()
        .uid(&onchain.contracts().domain_separator, owner);
    let sig_bytes = order.signature.to_bytes();
    let deadline = (chrono::Utc::now() + chrono::Duration::seconds(3)).to_rfc3339();

    let response = http
        .post(format!("{driver_url}/solve"))
        .header("X-Auction-Id", auction_id.to_string())
        .json(&serde_json::json!({
            "id": auction_id.to_string(),
            "tokens": [
                {"address": format!("{weth:?}"), "price": "1000000000000000000", "trusted": true},
                {"address": format!("{:?}", order.buy_token), "price": "1000000000000000000", "trusted": false},
            ],
            "orders": [{
                "uid": format!("{uid}"),
                "sellToken": format!("{:?}", order.sell_token),
                "buyToken": format!("{:?}", order.buy_token),
                "sellAmount": order.sell_amount.to_string(),
                "buyAmount": order.buy_amount.to_string(),
                "protocolFees": [],
                "created": model::time::now_in_epoch_seconds(),
                "validTo": order.valid_to,
                "kind": "buy",
                "receiver": null,
                "owner": format!("{owner:?}"),
                "partiallyFillable": false,
                "executed": "0",
                "preInteractions": [],
                "postInteractions": [],
                "class": "market",
                "appData": format!("0x{}", const_hex::encode([0u8; 32])),
                "signingScheme": "eip712",
                "signature": format!("0x{}", const_hex::encode(&sig_bytes)),
            }],
            "deadline": deadline,
        }))
        .send()
        .await
        .expect("failed to send /solve request");

    let status = response.status();
    let text = response
        .text()
        .await
        .expect("failed to read /solve response");
    assert!(status.is_success(), "/solve failed ({status}): {text}");

    let body: serde_json::Value = serde_json::from_str(&text).expect("invalid JSON from /solve");

    let solution_id = body["solutions"][0]["solutionId"]
        .as_u64()
        .unwrap_or_else(|| panic!("no solutionId in /solve response: {text}"));

    tracing::info!(auction_id, solution_id, "solve succeeded");
    solution_id
}

async fn assert_settle_success(
    settle_result: Result<reqwest::Result<reqwest::Response>, tokio::task::JoinError>,
    auction_id: &str,
) {
    let response = settle_result
        .unwrap_or_else(|err| panic!("/settle task for auction {auction_id} panicked: {err}"))
        .unwrap_or_else(|err| panic!("/settle request for auction {auction_id} failed: {err}"));
    let status = response.status();
    let body = response.text().await.unwrap_or_else(|err| {
        panic!("failed to read /settle response for auction {auction_id}: {err}")
    });
    assert!(
        status.is_success(),
        "/settle failed for auction {auction_id} ({status}): {body}"
    );
}

/// Deploy the CowSettlementForwarder contract (target-agnostic, with caller
/// whitelist). No constructor arguments — storage lives in the delegating EOA.
async fn deploy_forwarder(web3: &Web3, deployer: &TestAccount) -> Address {
    CowSettlementForwarder::deploy_builder(web3.provider.clone())
        .from(deployer.address())
        .deploy()
        .await
        .expect("failed to deploy CowSettlementForwarder")
}

/// Set up EIP-7702 delegation on the `solver` EOA, pointing to `forwarder`.
/// Sends a transaction from `submitter` with the signed authorization.
async fn setup_eip7702_delegation(
    web3: &Web3,
    solver: &TestAccount,
    submitter: &TestAccount,
    forwarder: Address,
) {
    let chain_id = web3
        .provider
        .get_chain_id()
        .await
        .expect("failed to get chain_id");
    let solver_nonce = solver.nonce(web3).await;

    let auth = Authorization {
        chain_id: U256::from(chain_id),
        address: forwarder,
        nonce: solver_nonce,
    };

    let sig = solver
        .signer
        .sign_hash(&auth.signature_hash())
        .await
        .expect("failed to sign EIP-7702 authorization");
    let signed_auth = auth.into_signed(sig);

    // Send a self-transfer from the submitter that carries the authorization
    // list. Once mined, the solver EOA's code will delegate to the forwarder.
    let tx = TransactionRequest::default()
        .from(submitter.address())
        .to(submitter.address())
        .value(U256::ZERO)
        .with_authorization_list(vec![signed_auth]);

    web3.provider
        .send_transaction(tx)
        .await
        .expect("failed to send EIP-7702 delegation tx")
        .get_receipt()
        .await
        .expect("EIP-7702 delegation tx failed");
}

/// Approve submission EOAs as callers on the forwarder. The solver signs
/// a self-call — after 7702 delegation, `msg.sender == address(this)` passes
/// the auth check in `setApprovedCallers`.
async fn approve_submission_callers(web3: &Web3, solver: &TestAccount, callers: &[Address]) {
    let data = CowSettlementForwarder::setApprovedCallersCall {
        callers: callers.to_vec(),
        approved: true,
    }
    .abi_encode();

    let tx = TransactionRequest::default()
        .from(solver.address())
        .to(solver.address())
        .input(data.into());

    web3.provider
        .send_transaction(tx)
        .await
        .expect("failed to send setApprovedCallers tx")
        .get_receipt()
        .await
        .expect("setApprovedCallers tx failed");
}

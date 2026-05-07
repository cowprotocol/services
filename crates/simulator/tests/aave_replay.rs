use {
    alloy_primitives::{U256, address, hex},
    app_data::{AppDataHash, hash_full_app_data},
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::Signature,
    },
    simulator::simulation_builder::{
        self,
        Block,
        EthCallInputs,
        ExecutionAmount,
        PriceEncoding,
        SettlementSimulator,
        Solver,
    },
    std::{str::FromStr, sync::Arc},
};

/// Full `app_data` JSON the trader signed for the replayed Aave v3 debt-swap
/// order. Source-level whitespace is for readability only - run through
/// `canonicalise_app_data` before hashing or passing downstream.
const APP_DATA: &str = r#"{
    "appCode": "aave-v3-interface-debt-swap",
    "metadata": {
        "flashloan": {
            "amount": "4475596734006878742",
            "liquidityProvider": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
            "protocolAdapter": "0xdeCC46a4b09162F5369c5C80383AAa9159bCf192",
            "receiver": "0xdeCC46a4b09162F5369c5C80383AAa9159bCf192",
            "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        },
        "hooks": {
            "post": [{
                "callData": "0xad3da559000000000000000000000000000000000000000000000000444d51cbc68377680000000000000000000000000000000000000000000000000000000069f323f8000000000000000000000000000000000000000000000000000000000000001c445675473b3e0941842eb5405ec1d9cb93c7d64b513b30d928d7ea42067440cb00fbafa80085d754964d5d70f616d899049f4e75c240fda7c2f108a99c882e8b",
                "dappId": "cow-sdk://flashloans/aave/v3/debt-swap",
                "gasLimit": "1000000",
                "target": "0xe58aCB86761699c1cBC665e6b7E0271503f6336C"
            }],
            "pre": [{
                "callData": "0xb1b6308b00000000000000000000000073e7af13ef172f13d8fefebfd90c7a65300963440000000000000000000000006276ac03090f2bb8be680178343ac368f713b4e8000000000000000000000000e58acb86761699c1cbc665e6b7e0271503f6336c000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000040d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f0000000000000000000000000000000000000000000000003e14904047a25ee600000000000000000000000000000000000000000000021e4382edd5a86c00006ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc0000000000000000000000000000000000000000000000000000000069f323f80000000000000000000000000000000000000000000000003e1c83845060e2160000000000000000000000000000000000000000000000000007f34408be83300000000000000000000000000000000000000000000000003e1c83845060e21600000000000000000000000000000000000000000000021e4382edd5a86c0000",
                "dappId": "cow-sdk://flashloans/aave/v3/debt-swap",
                "gasLimit": "300000",
                "target": "0xdeCC46a4b09162F5369c5C80383AAa9159bCf192"
            }]
        },
        "orderClass": {"orderClass": "market"},
        "partnerFee": {"recipient": "0x464C71f6c2F760DdA6093dCB91C24c39e5d6e18c", "volumeBps": 0},
        "quote": {"slippageBips": 140, "smartSlippage": true},
        "utm": {
            "utmCampaign": "developer-cohort",
            "utmContent": "",
            "utmMedium": "cow-sdk@7.3.4",
            "utmSource": "cowmunity",
            "utmTerm": "js"
        }
    },
    "version": "1.14.0"
}"#;

/// Returns `app_data` minified with object keys sorted alphabetically.
///
/// The sort comes from `serde_json::Value::Object`'s `BTreeMap` backing, which
/// applies whenever the `preserve_order` feature is not enabled (it isn't in
/// this workspace). The signed production payload happens to already be
/// alphabetically keyed at every level, so the output matches it byte for
/// byte.
fn canonicalise_app_data(app_data: &str) -> String {
    let value: serde_json::Value =
        serde_json::from_str(app_data).expect("APP_DATA must be valid JSON");
    serde_json::to_string(&value).expect("re-serialising must succeed")
}

/// Production EIP-1271 signature blob for the replayed order. The trader's
/// signer contract decodes it and validates against the order hash.
const SIGNATURE_HEX: &str = "0x000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000040d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f000000000000000000000000e58acb86761699c1cbc665e6b7e0271503f6336c0000000000000000000000000000000000000000000000003e14904047a25ee600000000000000000000000000000000000000000000021e4382edd5a86c00000000000000000000000000000000000000000000000000000000000069f323f8a1435054976e030f531f620f051bbabe34ef387901808b8677cf7c9304c21f3c00000000000000000000000000000000000000000000000000000000000000006ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc00000000000000000000000000000000000000000000000000000000000000005a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc95a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc900000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000041bb5488854dd5149f8843514851b7e25499917ca742af77061d2355681f3b608157bb34a59f9632e2228ea869c6d571f822295ee2eb03904dd8dd874245478f3b1b00000000000000000000000000000000000000000000000000000000000000";

/// Full `app_data` JSON for a real production AAVE debt-swap order
/// (`0x82ce5971...69f143f4`) that participated in 9 mainnet auctions on
/// 2026-04-28 and never settled. From the autopilot's view, every attempt
/// ended with `err: Timeout` (kipseli kept winning but couldn't submit
/// before the deadline). USDT -> 0x6c3e..., 3bps slippage with smartSlippage.
const REAL_FAILED_APP_DATA: &str = r#"{"appCode":"aave-v3-interface-debt-swap","metadata":{"flashloan":{"amount":"10018240625","liquidityProvider":"0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2","protocolAdapter":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192","receiver":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192","token":"0xdac17f958d2ee523a2206206994597c13d831ec7"},"hooks":{"post":[{"callData":"0xad3da5590000000000000000000000000000000000000000000000000000000290a6696b0000000000000000000000000000000000000000000000000000000069f143f4000000000000000000000000000000000000000000000000000000000000001b9e71e40b50fd28380bff2592c39d658f23fcce918143cbcd3aa15dea4a67e812076a30c76def1594b85b4cb44b2fcad15617a6f39778a1802c96587950390a7f","dappId":"cow-sdk://flashloans/aave/v3/debt-swap","gasLimit":"700000","target":"0x6af596e3ef71a12192B6861D66B0887b3Be39725"}],"pre":[{"callData":"0xb1b6308b00000000000000000000000073e7af13ef172f13d8fefebfd90c7a6530096344000000000000000000000000409b163381308ac8ad0a434098671442d10f5cd90000000000000000000000006af596e3ef71a12192b6861d66b0887b3be39725000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec70000000000000000000000006c3ea9036406852006290770bedfcaba0e23a0e80000000000000000000000000000000000000000000000000000000254d5c990000000000000000000000000000000000000000000000000000000025439aac06ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc0000000000000000000000000000000000000000000000000000000069f143f4000000000000000000000000000000000000000000000000000000025522387100000000000000000000000000000000000000000000000000000000004c6ee10000000000000000000000000000000000000000000000000000000255223871000000000000000000000000000000000000000000000000000000025439aac0","dappId":"cow-sdk://flashloans/aave/v3/debt-swap","gasLimit":"300000","target":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192"}]},"orderClass":{"orderClass":"market"},"partnerFee":{"recipient":"0x464C71f6c2F760DdA6093dCB91C24c39e5d6e18c","volumeBps":0},"quote":{"slippageBips":3,"smartSlippage":true},"utm":{"utmCampaign":"developer-cohort","utmContent":"","utmMedium":"cow-sdk@7.3.4","utmSource":"cowmunity","utmTerm":"js"}},"version":"1.14.0"}"#;

/// Production EIP-1271 signature blob for the real failing order
/// `0x82ce5971...69f143f4`.
const REAL_FAILED_SIGNATURE_HEX: &str = "0x000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec70000000000000000000000006c3ea9036406852006290770bedfcaba0e23a0e80000000000000000000000006af596e3ef71a12192b6861d66b0887b3be397250000000000000000000000000000000000000000000000000000000254d5c990000000000000000000000000000000000000000000000000000000025439aac00000000000000000000000000000000000000000000000000000000069f143f4a12ce0ec5cacaae18d9a1a0ac7e1d6d735ddc633082c51abd0d9198d3e15800000000000000000000000000000000000000000000000000000000000000000006ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc00000000000000000000000000000000000000000000000000000000000000005a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc95a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc900000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000041a83a7be699c08141a8bc246fba919068a727756f09aa7a034320df48f8294ca26d2537769285a0297522216bbd6fa45f92cef48139a5e5cb4cbaa0b4db8985011b00000000000000000000000000000000000000000000000000000000000000";

/// Replay of a real Aave v3 debt-swap order against a historical mainnet
/// block, exercising the prototype's `SettlementSimulator`-based simulation
/// path end-to-end without involving the orderbook's wall-clock validity
/// check.
///
/// Why this test exists:
///
/// - A full `OrderValidator::validate_and_construct_order` flow uses
///   `SystemTime::now()` to bound `valid_to`, which makes any historical order
///   replay non-deterministic (the test rots as the order expires).
/// - The prototype's value lives in the simulation, not the validity check, so
///   we exercise the simulation directly: build a `SettlementSimulator` against
///   a real RPC, pin the simulation to the block right before settlement, and
///   assert it does not revert.
///
/// Order replayed: an `aave-v3-interface-debt-swap` order
/// `0x7f5df255b55f5eba3034f74acb8e91a04aaf61a755b88c61ad7c61068856f3b2e58acb86761699c1cbc665e6b7e0271503f6336c69f323f8`,
/// sell WETH, buy GHO. The owner is an EIP-1167 minimal proxy that the
/// pre-hook deploys just-in-time.
#[tokio::test]
#[ignore]
async fn aave_debt_swap_replay() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let canonical_app_data = canonicalise_app_data(APP_DATA);
    let inputs = build_replay_simulation(&rpc_url, &canonical_app_data).await;

    inputs
        .simulate()
        .await
        .expect("simulation must not revert for a healthy production order");
}

/// Same order, but the `flashloan.amount` in `app_data` is rewritten to a
/// value Aave's WETH pool cannot satisfy. The wrapper call to the Aave Pool
/// must revert, and the simulation must propagate that revert.
///
/// This proves the prototype actually executes the flashloan path: if the
/// wrapper call were a silent no-op (e.g. wrong router address), the
/// simulation would not depend on Aave's liquidity at all and would not
/// fail here.
#[tokio::test]
#[ignore]
async fn aave_debt_swap_replay_fails_when_flashloan_oversubscribed() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let mut value: serde_json::Value =
        serde_json::from_str(APP_DATA).expect("APP_DATA must be valid JSON");
    // Way more WETH than Aave can lend. Aave reverts with insufficient
    // liquidity (or similar) before any settlement runs.
    value["metadata"]["flashloan"]["amount"] = serde_json::Value::String(U256::MAX.to_string());
    let tampered_app_data = serde_json::to_string(&value).unwrap();

    let inputs = build_replay_simulation(&rpc_url, &tampered_app_data).await;

    let err = inputs
        .simulate()
        .await
        .expect_err("simulation must revert when the flashloan exceeds Aave liquidity");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("execution reverted"),
        "expected an EVM revert, got: {msg}",
    );
}

/// Replay of a real production AAVE debt-swap order
/// (`0x82ce5971...69f143f4`) that participated in 9 mainnet auctions on
/// 2026-04-28 and never settled. Pinned to a block inside the order's
/// active window (first `Ready` event was at 2026-04-28 23:24:38 UTC,
/// block 24_981_834).
///
/// This test exists to surface what `SettlementSimulator` actually sees for
/// a real production order whose autopilot-side failure mode was `Timeout`
/// across every attempt. `Timeout` is a coordination/submission failure and
/// is not an EVM revert, so it is not directly reproducible in `eth_call`.
/// Two outcomes are expected:
///
/// - The simulation succeeds, confirming the prod failure was timing /
///   submission related (mempool, deadline, RPC) rather than a deterministic
///   on-chain revert.
/// - The simulation reverts, in which case the printed error tells us which
///   step failed and we can promote this into a hard-asserting negative test.
///
/// Either outcome is informative, so the test does not enforce a specific
/// result. It panics only if the simulation builder cannot wire the order.
#[tokio::test]
#[ignore]
async fn aave_debt_swap_replay_real_failed_order() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let canonical_app_data = canonicalise_app_data(REAL_FAILED_APP_DATA);
    let inputs = build_real_failed_replay_simulation(&rpc_url, &canonical_app_data).await;

    match inputs.simulate().await {
        Ok(_) => eprintln!(
            "[real failed order replay] simulation SUCCEEDED. Production failure was likely a \
             timeout / submission issue, not a deterministic EVM revert."
        ),
        Err(err) => eprintln!("[real failed order replay] simulation REVERTED: {err:?}"),
    }
}

/// Builds a simulation pinned to the block right before the Aave debt-swap
/// settlement. The caller controls `full_app_data` so the same wiring
/// supports a positive replay (untouched) and a negative replay (tampered).
async fn build_replay_simulation(rpc_url: &str, full_app_data: &str) -> EthCallInputs {
    // One block before the on-chain settlement transaction. At this block
    // the helper-clone owner contract has no code yet (the pre-hook deploys
    // it), the protocol-adapter factory is live, and Aave v3 has WETH
    // liquidity.
    let fork_block_mainnet = 24_992_051u64;
    let order_owner = address!("e58aCB86761699c1cBC665e6b7E0271503f6336C");
    let sell_token_weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let buy_token_gho = address!("40d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f");
    let sell_amount = U256::from_str("4473358935639875302").unwrap();
    let buy_amount = U256::from_str("10003000000000000000000").unwrap();
    let valid_to = 1_777_542_136u32; // 2026-04-30 09:42:16 UTC

    let web3 = ethrpc::Web3::new_from_url(rpc_url);
    let provider = web3.provider.clone();
    let chain_id = 1u64;

    let settlement = contracts::GPv2Settlement::Instance::deployed(&provider)
        .await
        .expect("settlement contract not deployed on mainnet?");

    let flash_loan_router = contracts::FlashLoanRouter::deployment_address(&chain_id)
        .expect("FlashLoanRouter deployment address");
    let hooks_trampoline = contracts::HooksTrampoline::deployment_address(&chain_id)
        .expect("HooksTrampoline deployment address");

    let balance_overrider = Arc::new(balance_overrides::BalanceOverrides::new(web3));
    let block_stream = ethrpc::block_stream::mock_single_block(Default::default());

    let simulator = SettlementSimulator::new(
        settlement,
        flash_loan_router,
        hooks_trampoline,
        sell_token_weth,
        30_000_000u64,
        balance_overrider,
        block_stream,
        None,
    )
    .await
    .expect("failed to create SettlementSimulator");

    let signature_bytes = hex::decode(SIGNATURE_HEX.trim_start_matches("0x"))
        .expect("SIGNATURE_HEX must be valid hex");
    let app_data_hash = AppDataHash(hash_full_app_data(full_app_data.as_bytes()));

    let order_data = OrderData {
        sell_token: sell_token_weth,
        buy_token: buy_token_gho,
        receiver: Some(order_owner),
        sell_amount,
        buy_amount,
        valid_to,
        app_data: app_data_hash,
        fee_amount: U256::ZERO,
        kind: OrderKind::Buy,
        partially_fillable: false,
        sell_token_balance: SellTokenSource::Erc20,
        buy_token_balance: BuyTokenDestination::Erc20,
    };

    simulator
        .new_simulation_builder()
        .with_orders([simulation_builder::Order::new(order_data)
            .with_signature(order_owner, Signature::Eip1271(signature_bytes))
            .fill_at(ExecutionAmount::Full, PriceEncoding::LimitPrice)])
        .parameters_from_app_data(full_app_data)
        .expect("parameters_from_app_data should parse the app data")
        .from_solver(Solver::Fake(None))
        .provide_sufficient_buy_tokens()
        .at_block(Block::Number(fork_block_mainnet))
        .build()
        .await
        .expect("failed to build simulation")
}

/// Builds a simulation for the real production debt-swap order
/// `0x82ce5971...69f143f4` pinned to a block inside its active window.
///
/// Owner is the trader's already-deployed EIP-1271 signer (different from
/// the JIT-deployed proxy pattern used by `build_replay_simulation`).
async fn build_real_failed_replay_simulation(rpc_url: &str, full_app_data: &str) -> EthCallInputs {
    // Mainnet block at 2026-04-28 23:24:38 UTC, the timestamp of the order's
    // first `Ready` event. Order's full active window spans ~5 minutes ending
    // just before `valid_to` (2026-04-28 23:34:12 UTC).
    let fork_block_mainnet = 24_981_834u64;
    let order_owner = address!("6af596e3ef71a12192b6861d66b0887b3be39725");
    let sell_token_usdt = address!("dac17f958d2ee523a2206206994597c13d831ec7");
    let buy_token = address!("6c3ea9036406852006290770bedfcaba0e23a0e8");
    let sell_amount = U256::from_str("10013231504").unwrap();
    let buy_amount = U256::from_str("10003000000").unwrap();
    let valid_to = 1_777_419_252u32; // 2026-04-28 23:34:12 UTC

    let web3 = ethrpc::Web3::new_from_url(rpc_url);
    let provider = web3.provider.clone();
    let chain_id = 1u64;

    let settlement = contracts::GPv2Settlement::Instance::deployed(&provider)
        .await
        .expect("settlement contract not deployed on mainnet?");

    let flash_loan_router = contracts::FlashLoanRouter::deployment_address(&chain_id)
        .expect("FlashLoanRouter deployment address");
    let hooks_trampoline = contracts::HooksTrampoline::deployment_address(&chain_id)
        .expect("HooksTrampoline deployment address");

    let balance_overrider = Arc::new(balance_overrides::BalanceOverrides::new(web3));
    let block_stream = ethrpc::block_stream::mock_single_block(Default::default());

    let simulator = SettlementSimulator::new(
        settlement,
        flash_loan_router,
        hooks_trampoline,
        sell_token_usdt,
        30_000_000u64,
        balance_overrider,
        block_stream,
        None,
    )
    .await
    .expect("failed to create SettlementSimulator");

    let signature_bytes = hex::decode(REAL_FAILED_SIGNATURE_HEX.trim_start_matches("0x"))
        .expect("REAL_FAILED_SIGNATURE_HEX must be valid hex");
    let app_data_hash = AppDataHash(hash_full_app_data(full_app_data.as_bytes()));

    let order_data = OrderData {
        sell_token: sell_token_usdt,
        buy_token,
        // The DB row did not expose the receiver field. Defaulting to
        // `Some(order_owner)` mirrors `build_replay_simulation`. If the
        // EIP-1271 signature check rejects the order, try `None` (which
        // EIP-712-encodes as zero address).
        receiver: Some(order_owner),
        sell_amount,
        buy_amount,
        valid_to,
        app_data: app_data_hash,
        fee_amount: U256::ZERO,
        kind: OrderKind::Buy,
        partially_fillable: false,
        sell_token_balance: SellTokenSource::Erc20,
        buy_token_balance: BuyTokenDestination::Erc20,
    };

    simulator
        .new_simulation_builder()
        .with_orders([simulation_builder::Order::new(order_data)
            .with_signature(order_owner, Signature::Eip1271(signature_bytes))
            .fill_at(ExecutionAmount::Full, PriceEncoding::LimitPrice)])
        .parameters_from_app_data(full_app_data)
        .expect("parameters_from_app_data should parse the app data")
        .from_solver(Solver::Fake(None))
        .provide_sufficient_buy_tokens()
        .at_block(Block::Number(fork_block_mainnet))
        .build()
        .await
        .expect("failed to build simulation")
}

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

/// Minifies `app_data` with keys sorted alphabetically.
fn canonicalise_app_data(app_data: &str) -> String {
    let value: serde_json::Value =
        serde_json::from_str(app_data).expect("APP_DATA must be valid JSON");
    serde_json::to_string(&value).expect("re-serialising must succeed")
}

/// Production EIP-1271 signature blob for the replayed order.
const SIGNATURE_HEX: &str = "0x000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000040d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f000000000000000000000000e58acb86761699c1cbc665e6b7e0271503f6336c0000000000000000000000000000000000000000000000003e14904047a25ee600000000000000000000000000000000000000000000021e4382edd5a86c00000000000000000000000000000000000000000000000000000000000069f323f8a1435054976e030f531f620f051bbabe34ef387901808b8677cf7c9304c21f3c00000000000000000000000000000000000000000000000000000000000000006ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc00000000000000000000000000000000000000000000000000000000000000005a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc95a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc900000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000041bb5488854dd5149f8843514851b7e25499917ca742af77061d2355681f3b608157bb34a59f9632e2228ea869c6d571f822295ee2eb03904dd8dd874245478f3b1b00000000000000000000000000000000000000000000000000000000000000";

/// AAVE v3 debt-swap order
/// `0x7f5df255b55f5eba3034f74acb8e91a04aaf61a755b88c61ad7c61068856f3b2e58acb86761699c1cbc665e6b7e0271503f6336c69f323f8`,
/// owner is an EIP-1167 minimal proxy the pre-hook deploys JIT.
///
/// We exercise `SettlementSimulator` directly instead of going through
/// `OrderValidator::validate_and_construct_order`, which bounds `valid_to`
/// by `SystemTime::now()` and would make this test rot.
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

/// Shared builder for the positive replay (`APP_DATA`) and the
/// flashloan-tampered negative replay.
async fn build_replay_simulation(rpc_url: &str, full_app_data: &str) -> EthCallInputs {
    // Pinned one block before the on-chain settlement: pre-hook hasn't
    // yet deployed the owner contract, Aave has WETH liquidity.
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

/// AAVE v3 collateral-swap order
/// `0x441ad034a3c8cd9ad0fc9a9d143c8201bc92d62851a8428997c36a89a03ee2ad4caea9074f2897a3a4ac173c4e5b5bd8b7e3dc976b5f9c6f`
/// which reverts onchain due to corrupted hooks
const NATURALLY_FAILING_APP_DATA: &str = r#"{"appCode":"aave-v3-interface-collateral-swap","metadata":{"flashloan":{"amount":"6136714","liquidityProvider":"0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2","protocolAdapter":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192","receiver":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192","token":"0x2260fac5e5542a773aa44fbcfedf7c193bc2c599"},"hooks":{"post":[{"callData":"0x398925de00000000000000000000000000000000000000000000000000000000006700b10000000000000000000000000000000000000000000000000000000069850ebf000000000000000000000000000000000000000000000000000000000000001bb32da7ed8403b8369956ffc15f4c1295953004866fa786c0162d28bea3d18b3b08466a876f9a0cb5d1175ef44838426558b19d64b48002231a8c1c90821e08a4","dappId":"cow-sdk://flashloans/aave/v3/collateral-swap","gasLimit":"700000","target":"0x4CAea9074f2897a3A4ac173C4E5b5Bd8b7E3Dc97"}],"pre":[{"callData":"0xb1b6308b000000000000000000000000029d584e847373b6373b01dfad1a0c9bfb9163820000000000000000000000004ec7efb8c873f54c5f62830ec5ecc2362580bdfe0000000000000000000000004caea9074f2897a3a4ac173c4e5b5bd8b7e3dc970000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec700000000000000000000000000000000000000000000000000000000005d978d00000000000000000000000000000000000000000000000000000000c9f769e0f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775000000000000000000000000000000000000000000000000000000006b5f9c6f00000000000000000000000000000000000000000000000000000000005da38a0000000000000000000000000000000000000000000000000000000000000bfd00000000000000000000000000000000000000000000000000000000005da38a00000000000000000000000000000000000000000000000000000000c9f769e0","dappId":"cow-sdk://flashloans/aave/v3/collateral-swap","gasLimit":"300000","target":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192"}]},"orderClass":{"orderClass":"limit"},"partnerFee":{"recipient":"0xC542C2F197c4939154017c802B0583C596438380","volumeBps":25},"quote":{"slippageBips":0,"smartSlippage":true},"utm":{"utmCampaign":"developer-cohort","utmContent":"","utmMedium":"cow-sdk@7.3.4","utmSource":"cowmunity","utmTerm":"js"}},"version":"1.14.0"}"#;

const NATURALLY_FAILING_SIGNATURE_HEX: &str = "0x0000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec70000000000000000000000004caea9074f2897a3a4ac173c4e5b5bd8b7e3dc9700000000000000000000000000000000000000000000000000000000005d978d00000000000000000000000000000000000000000000000000000000c9f769e0000000000000000000000000000000000000000000000000000000006b5f9c6f4223823feadc36c4373c9d88ac1e9875d067e3df5ced70c0a1fedcec396f34d10000000000000000000000000000000000000000000000000000000000000000f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677500000000000000000000000000000000000000000000000000000000000000005a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc95a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc900000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000041dd46f4ee5ab3d65846780582c116a93cf37d99be5c87ed1638e3f52bc39861b91595d66c1afd17d585d6ccdbe7b643a2d44b46d1a919f479224e49934c293f231c00000000000000000000000000000000000000000000000000000000000000";

/// Replay of a real production order that the mainnet driver was repeatedly
/// dropping with `Simulation(Revert(_))`. The pre-hook reverts because the
/// trader's aToken balance is insufficient for the collateral swap.
#[tokio::test]
#[ignore]
async fn aave_collateral_swap_replay_fails_naturally() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let canonical_app_data = canonicalise_app_data(NATURALLY_FAILING_APP_DATA);
    let inputs = build_naturally_failing_replay_simulation(&rpc_url, &canonical_app_data).await;

    let err = inputs
        .simulate()
        .await
        .expect_err("simulation must revert: pre-hook moves aToken the trader no longer holds");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("execution reverted"),
        "expected an EVM revert, got: {msg}",
    );
}

async fn build_naturally_failing_replay_simulation(
    rpc_url: &str,
    full_app_data: &str,
) -> EthCallInputs {
    // Block taken from a `BlockNo(...)` embedded in one of the dropped-
    // solution events for this order on 2026-05-05.
    let fork_block_mainnet = 25_028_258u64;
    let chain_id = 1u64;
    let order_owner = address!("4caea9074f2897a3a4ac173c4e5b5bd8b7e3dc97");
    let sell_token_a_wbtc = address!("2260fac5e5542a773aa44fbcfedf7c193bc2c599");
    let buy_token_usdt = address!("dac17f958d2ee523a2206206994597c13d831ec7");
    let sell_amount = U256::from_str("6133645").unwrap();
    let buy_amount = U256::from_str("3388434912").unwrap();
    let valid_to = 1_801_428_079u32; // 2027-01-31 ish

    let web3 = ethrpc::Web3::new_from_url(rpc_url);
    let provider = web3.provider.clone();

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
        sell_token_a_wbtc,
        30_000_000u64,
        balance_overrider,
        block_stream,
        None,
    )
    .await
    .expect("failed to create SettlementSimulator");

    let signature_bytes = hex::decode(NATURALLY_FAILING_SIGNATURE_HEX.trim_start_matches("0x"))
        .expect("NATURALLY_FAILING_SIGNATURE_HEX must be valid hex");
    let app_data_hash = AppDataHash(hash_full_app_data(full_app_data.as_bytes()));

    let order_data = OrderData {
        sell_token: sell_token_a_wbtc,
        buy_token: buy_token_usdt,
        receiver: Some(order_owner),
        sell_amount,
        buy_amount,
        valid_to,
        app_data: app_data_hash,
        fee_amount: U256::ZERO,
        kind: OrderKind::Sell,
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

//! Replay of a real production AAVE v3 collateral-swap order on polygon
//! that deterministically failed simulation. Source: zeroex-solve on
//! polygon was firing the `driver_dropped_solutions_error_rate` alert on
//! 2026-05-05. Order
//! `0x23e6c810f0fdf4bd5566be0b106fdd53700d575f20a0ad721ea397a3eda0a67a515c8ea095e8bd231f68d66ff6ef60005275b6306b69c8a4`
//! was attempted ~290 times in a
//! single day and dropped at the same step every time with
//! `Simulation(Revert(RevertError { err: Ethereum(AccessList("execution
//! reverted")) }))`. Replaying it pinned to block 86_452_778 (2026-05-05
//! 12:00 UTC) reproduces the underlying on-chain revert: the pre-hook
//! tries to move the trader's `aPolUSDCn` (sell aToken) balance into the
//! protocol adapter, and at this block the trader doesn't hold enough.

mod common;

use {
    alloy_primitives::{U256, address, hex},
    app_data::{AppDataHash, hash_full_app_data},
    common::canonicalise_app_data,
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

/// Full `app_data` JSON the trader signed for the replayed order.
const APP_DATA: &str = r#"{"appCode":"aave-v3-interface-collateral-swap","metadata":{"flashloan":{"amount":"500733","liquidityProvider":"0x794a61358D6845594F94dc1DB02A252b5b4814aD","protocolAdapter":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192","receiver":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192","token":"0x1bfd67037b42cf73acf2047067bd4f2c47d9bfd6"},"hooks":{"post":[{"callData":"0x398925de000000000000000000000000000000000000000000000000000000000008679600000000000000000000000000000000000000000000000000000000698f3afb000000000000000000000000000000000000000000000000000000000000001b55d3ce98924e9a673cbebea6ba4e9a35a0148ed34638d0790d2830a4a2adea0900a4b3dbc907dfae9d5a4419db57ed48e6bdf6ca9c29c8d430ecde47bceb9288","dappId":"cow-sdk://flashloans/aave/v3/collateral-swap","gasLimit":"1200000","target":"0x515C8EA095E8bd231F68d66fF6Ef60005275B630"}],"pre":[{"callData":"0xb1b6308b000000000000000000000000029d584e847373b6373b01dfad1a0c9bfb9163820000000000000000000000002e7bca3ac237903bce134d5bd88145f50f9ad794000000000000000000000000515c8ea095e8bd231f68d66ff6ef60005275b6300000000000000000000000001bfd67037b42cf73acf2047067bd4f2c47d9bfd60000000000000000000000003c499c542cef5e3811e1192ce70d8cc03d5c3359000000000000000000000000000000000000000000000000000000000007a302000000000000000000000000000000000000000000000000000000000db34218f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775000000000000000000000000000000000000000000000000000000006b69c8a4000000000000000000000000000000000000000000000000000000000007a3fd00000000000000000000000000000000000000000000000000000000000000fb000000000000000000000000000000000000000000000000000000000007a3fd000000000000000000000000000000000000000000000000000000000db34218","dappId":"cow-sdk://flashloans/aave/v3/collateral-swap","gasLimit":"300000","target":"0xdeCC46a4b09162F5369c5C80383AAa9159bCf192"}]},"orderClass":{"orderClass":"limit"},"partnerFee":{"recipient":"0xC542C2F197c4939154017c802B0583C596438380","volumeBps":25},"quote":{"slippageBips":0,"smartSlippage":true},"utm":{"utmCampaign":"developer-cohort","utmContent":"","utmMedium":"cow-sdk@7.3.4","utmSource":"cowmunity","utmTerm":"js"}},"version":"1.14.0"}"#;

/// Production EIP-1271 signature blob for the replayed order.
const SIGNATURE_HEX: &str = "0x0000000000000000000000001bfd67037b42cf73acf2047067bd4f2c47d9bfd60000000000000000000000003c499c542cef5e3811e1192ce70d8cc03d5c3359000000000000000000000000515c8ea095e8bd231f68d66ff6ef60005275b630000000000000000000000000000000000000000000000000000000000007a302000000000000000000000000000000000000000000000000000000000db34218000000000000000000000000000000000000000000000000000000006b69c8a4acb672df38626fd2ad33c8bcfbc7789c27dec318d9883b968536717140d9344b0000000000000000000000000000000000000000000000000000000000000000f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677500000000000000000000000000000000000000000000000000000000000000005a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc95a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc900000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000041d4d6a9a1c8dce868ce2b5c0ccf46894fd8480add8ad3930ae8cbaebe4f40c6b75b4928c4e67c9dd4468d0828a3709d0ea65f4d20f2cf1ecc3253274f5a1aa32d1c00000000000000000000000000000000000000000000000000000000000000";

#[tokio::test]
#[ignore]
async fn aave_collateral_swap_zeroex_polygon_replay() {
    let Ok(rpc_url) = std::env::var("POLYGON_RPC_URL") else {
        eprintln!("POLYGON_RPC_URL not set - skipping replay test");
        return;
    };

    let canonical_app_data = canonicalise_app_data(APP_DATA);
    let inputs = build_replay_simulation(&rpc_url, &canonical_app_data).await;

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

async fn build_replay_simulation(rpc_url: &str, full_app_data: &str) -> EthCallInputs {
    // Polygon block 86_452_778 = 2026-05-05 12:00 UTC, inside the order's
    // active window. Polygon-bor full nodes typically don't keep archive
    // history this far back, so `POLYGON_RPC_URL` should point at a polygon
    // archive (e.g. Alchemy).
    let fork_block_polygon = 86_452_778u64;
    let chain_id = 137u64;
    let order_owner = address!("515c8ea095e8bd231f68d66ff6ef60005275b630");
    let sell_token_a_pol_usdc_n = address!("1bfd67037b42cf73acf2047067bd4f2c47d9bfd6");
    let buy_token_usdc_n = address!("3c499c542cef5e3811e1192ce70d8cc03d5c3359");
    let sell_amount = U256::from_str("500482").unwrap();
    let buy_amount = U256::from_str("229851672").unwrap();
    let valid_to = 1_802_094_756u32; // 2027-02 ish

    let web3 = ethrpc::Web3::new_from_url(rpc_url);
    let provider = web3.provider.clone();

    let settlement = contracts::GPv2Settlement::Instance::deployed(&provider)
        .await
        .expect("settlement contract not deployed on polygon?");

    let flash_loan_router = contracts::FlashLoanRouter::deployment_address(&chain_id)
        .expect("FlashLoanRouter deployment address (polygon)");
    let hooks_trampoline = contracts::HooksTrampoline::deployment_address(&chain_id)
        .expect("HooksTrampoline deployment address (polygon)");

    let balance_overrider = Arc::new(balance_overrides::BalanceOverrides::new(web3));
    let block_stream = ethrpc::block_stream::mock_single_block(Default::default());

    let simulator = SettlementSimulator::new(
        settlement,
        flash_loan_router,
        hooks_trampoline,
        sell_token_a_pol_usdc_n,
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
        sell_token: sell_token_a_pol_usdc_n,
        buy_token: buy_token_usdc_n,
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
        .at_block(Block::Number(fork_block_polygon))
        .build()
        .await
        .expect("failed to build simulation")
}

use {
    ::alloy::primitives::Address,
    contracts::IERC4626,
    e2e::setup::*,
    ethrpc::alloy::errors::ContractErrorExt,
    shared::web3::Web3,
    testlib::tokens::{BNT, GNO, STETH, SUSDE, USDC, WETH},
};

/// The block number from which we will fetch state for the forked test.
const FORK_BLOCK_MAINNET: u64 = 23112197;

/// Verifies that calling `asset()` on contracts that don't implement the
/// EIP-4626 selector is correctly classified as a contract revert by
/// `is_contract_revert`, regardless of how each node encodes the failure
/// (empty revert data, geth code 3, "execution reverted" message,
/// `EVM error InvalidFEOpcode`, ...).
#[tokio::test]
#[ignore]
async fn forked_node_mainnet_asset_call_is_contract_revert() {
    run_forked_test_with_block_number(
        asset_call_is_contract_revert_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

async fn asset_call_is_contract_revert_test(web3: Web3) {
    // sUSDe is a real EIP-4626 vault — `asset()` succeeds.
    let vault = IERC4626::IERC4626::new(SUSDE, &web3.provider);
    let asset = vault
        .asset()
        .call()
        .await
        .expect("asset() on sUSDe must succeed");
    assert_ne!(
        asset,
        Address::ZERO,
        "sUSDe asset() returned the zero address"
    );

    // For each non-vault contract, `asset()` must classify as a contract revert
    // (i.e. the call reached the contract and the contract rejected it because
    // it doesn't implement the selector — not a transport failure). Each entry
    // exercises a different failure shape: USDC is an empty-data revert, BNT
    // hits the INVALID (0xFE) opcode, the rest are plain reverts.
    for token in [USDC, WETH, BNT, STETH, GNO] {
        let vault = IERC4626::IERC4626::new(token, &web3.provider);
        let err = vault
            .asset()
            .call()
            .await
            .expect_err(&format!("asset() on {token} must fail"));
        assert!(
            err.is_contract_revert(),
            "asset() on {token} must classify as contract revert, got: {err:?}",
        );
    }
}

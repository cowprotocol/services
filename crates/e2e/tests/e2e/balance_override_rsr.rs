use {
    ::alloy::{
        primitives::{Address, U256, address},
        rpc::types::state::StateOverride,
    },
    balance_overrides::{
        ApprovalOverrideRequest,
        BalanceOverrideRequest,
        StateOverrides,
        StateOverriding,
    },
    contracts::ERC20,
    e2e::setup::*,
    shared::web3::Web3,
};

/// Recent block; the settlement contract crossed over on RSR years earlier.
const FORK_BLOCK_MAINNET: u64 = 26_200_000;

/// Reserve Rights token: copy-on-write over the old RSR contract.
const RSR: Address = address!("320623b8e4ff03373931769a31fc52a4e78b5d70");
const SETTLEMENT: Address = address!("9008D19f58AAbD9eD0D60971565AA8510560ab41");
const VAULT_RELAYER: Address = address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110");
/// Trader whose RSR allowance for the vault relayer has crossed, i.e. it was
/// set after the migration and lives in RSR's own storage.
const CROSSED_TRADER: Address = address!("74c3d4f8fa561e0130529d311cc372526dea5977");

/// RSR is copy-on-write over the old RSR contract: `balanceOf` returns
/// `_oldBal(account) + super.balanceOf(account)` until the account's first
/// transfer sets `balCrossed`, and `allowance` returns `oldRSR.allowance`
/// until the pair's first `approve` sets `allowanceCrossed`. Afterwards only
/// RSR's own storage is read. Strategies detected for uncrossed accounts must
/// still override crossed ones, like the settlement contract's balance or a
/// trader's post-migration allowance.
///
/// Verified source: <https://etherscan.io/address/0x320623b8e4ff03373931769a31fc52a4e78b5d70#code>
/// (`contracts/RSR.sol`: `balanceOf`, `_oldBal`, `ensureBalCrossed`,
/// `allowance`, `ensureAllowanceCrossed`).
/// GitHub: <https://github.com/reserve-protocol/rsr-mainnet/blob/059d5bc989748c40b48186fa809e50dda78e10b7/contracts/RSR.sol>
#[tokio::test]
#[ignore]
async fn forked_node_mainnet_balance_override_copy_on_write_token() {
    run_forked_test_with_block_number(
        balance_override_copy_on_write_token_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

async fn balance_override_copy_on_write_token_test(web3: Web3) {
    let overrides = StateOverrides::new(web3.clone());
    let amount = U256::from(123_456u128 * 10u128.pow(18));

    // A fresh trader never transferred RSR, so it is uncrossed. Quotes detect
    // the strategy with such holders first.
    let trader = Address::repeat_byte(0x42);
    assert_balance_override(&web3, &overrides, trader, amount).await;
    // The cached strategy is reused for the crossed settlement contract.
    assert_balance_override(&web3, &overrides, SETTLEMENT, amount).await;

    // Same for allowances: a fresh pair is uncrossed, the trader's is crossed.
    let fresh_owner = Address::repeat_byte(0x43);
    assert_allowance_override(&web3, &overrides, fresh_owner, VAULT_RELAYER, amount).await;
    assert_allowance_override(&web3, &overrides, CROSSED_TRADER, VAULT_RELAYER, amount).await;
}

async fn assert_allowance_override(
    web3: &Web3,
    overrides: &StateOverrides,
    owner: Address,
    spender: Address,
    amount: U256,
) {
    let (contract, account_override) = overrides
        .approval_override(ApprovalOverrideRequest {
            token: RSR,
            owner,
            spender,
            amount,
        })
        .await
        .expect("no approval override strategy detected for RSR");
    let mut state = StateOverride::default();
    state.insert(contract, account_override);
    let allowance = ERC20::ERC20::new(RSR, &web3.provider)
        .allowance(owner, spender)
        .state(state)
        .call()
        .await
        .unwrap();
    assert_eq!(
        allowance, amount,
        "override for {owner}/{spender} written to {contract} is not visible in allowance"
    );
}

async fn assert_balance_override(
    web3: &Web3,
    overrides: &StateOverrides,
    holder: Address,
    amount: U256,
) {
    let (contract, account_override) = overrides
        .balance_override(BalanceOverrideRequest {
            token: RSR,
            holder,
            amount,
        })
        .await
        .expect("no balance override strategy detected for RSR");
    let mut state = StateOverride::default();
    state.insert(contract, account_override);
    let balance = ERC20::ERC20::new(RSR, &web3.provider)
        .balanceOf(holder)
        .state(state)
        .call()
        .await
        .unwrap();
    assert_eq!(
        balance, amount,
        "override for {holder} written to {contract} is not visible in balanceOf"
    );
}

//! Helpers shared between the `BalanceOverrides` override builder and the
//! `Detector` probe/verify for Aave v3 aTokens.
//!
//! aTokens break the usual "balanceOf = storage[slot]" assumption twice:
//! - `balanceOf` returns `scaled_balance × getReserveNormalizedIncome / RAY`,
//!   not the raw slot value.
//! - Storage is packed `UserState { uint128 balance; uint128 additionalData }`
//!   in a single slot per holder.

use {
    alloy_primitives::{Address, B256, U256, keccak256},
    alloy_rpc_types::state::AccountOverride,
    alloy_sol_types::sol,
    ethrpc::Web3,
    std::iter,
};

sol! {
    /// Mirrors Aave v3's `DataTypes.ReserveConfigurationMap`.
    ///
    /// Source: <https://github.com/aave-dao/aave-v3-origin/blob/main/src/contracts/protocol/libraries/types/DataTypes.sol>
    struct ReserveConfigurationMap {
        uint256 data;
    }

    /// Mirrors Aave v3's `DataTypes.ReserveData`. Only the `aTokenAddress`
    /// field is consumed here; the rest are present to keep the ABI layout
    /// exactly matched so decoding doesn't go off the rails.
    ///
    /// Source: <https://github.com/aave-dao/aave-v3-origin/blob/main/src/contracts/protocol/libraries/types/DataTypes.sol>
    struct ReserveData {
        ReserveConfigurationMap configuration;
        uint128 liquidityIndex;
        uint128 currentLiquidityRate;
        uint128 variableBorrowIndex;
        uint128 currentVariableBorrowRate;
        uint128 currentStableBorrowRate;
        uint40 lastUpdateTimestamp;
        uint16 id;
        address aTokenAddress;
        address stableDebtTokenAddress;
        address variableDebtTokenAddress;
        address interestRateStrategyAddress;
        uint128 accruedToTreasury;
        uint128 unbacked;
        uint128 isolationModeTotalDebt;
    }

    /// Minimal interface for the Aave v3 `Pool`. `getReserveNormalizedIncome`
    /// gives the accrued liquidity index used when scaling `balanceOf`;
    /// `getReserveData` returns the full reserve record which we use to
    /// confirm a probed token really is the registered aToken for its
    /// underlying.
    ///
    /// Source: <https://github.com/aave-dao/aave-v3-origin/blob/main/src/contracts/interfaces/IPool.sol>
    #[sol(rpc)]
    interface IAaveV3Pool {
        function getReserveNormalizedIncome(address asset) external view returns (uint256);
        function getReserveData(address asset) external view returns (ReserveData memory);
    }

    /// Minimal interface for an Aave v3 `AToken`; used by the detector to
    /// decide whether a token is an aToken without any hardcoded list.
    ///
    /// Source: <https://github.com/aave-dao/aave-v3-origin/blob/main/src/contracts/interfaces/IAToken.sol>
    #[sol(rpc)]
    interface IAaveV3AToken {
        function UNDERLYING_ASSET_ADDRESS() external view returns (address);
        function POOL() external view returns (address);
    }
}

/// Ray (1e27) — Aave's 27-decimal fixed-point unit.
///
/// Source: <https://github.com/aave-dao/aave-v3-origin/blob/main/src/contracts/protocol/libraries/math/WadRayMath.sol>
pub const RAY: U256 = U256::from_limbs([0x9fd0803ce8000000, 0x33b2e3c, 0, 0]);

/// Storage slot index of `_userState` in the Aave v3 `IncentivizedERC20`
/// base contract. All canonical v3 aTokens inherit this layout, so the
/// detector can try this slot directly without a `debug_traceCall`.
///
/// Source: <https://github.com/aave-dao/aave-v3-origin/blob/main/src/contracts/protocol/tokenization/base/IncentivizedERC20.sol>
pub const USER_STATE_SLOT: u64 = 52;

/// Ray-division: `(a * RAY + b/2) / b`, round-half-up. Matches Aave's
/// `WadRayMath.rayDiv` bit-for-bit so the scaled amount we write into
/// storage equals the one Aave will itself compute during a subsequent
/// `_transfer`. Returns `None` if `b == 0` or the intermediate product
/// overflows `U256`.
pub fn ray_div(a: U256, b: U256) -> Option<U256> {
    if b.is_zero() {
        return None;
    }
    let half_b = b >> 1;
    a.checked_mul(RAY)
        .and_then(|prod| prod.checked_add(half_b))
        .map(|num| num / b)
}

/// `keccak256(pad32(holder) ++ map_slot)` — the storage slot of
/// `mapping(address => _)` entries in Solidity.
pub fn mapping_slot_hash(holder: &Address, map_slot: &[u8; 32]) -> B256 {
    let mut buf = [0u8; 64];
    buf[12..32].copy_from_slice(holder.as_slice());
    buf[32..64].copy_from_slice(map_slot);
    keccak256(buf)
}

/// Packs a `UserState { uint128 balance; uint128 additionalData }` into a
/// 32-byte word. The balance occupies the lower 128 bits; `additional_data`
/// sits in the upper 128 bits.
pub fn pack_user_state(balance: U256, additional_data: U256) -> B256 {
    let mask = (U256::from(1u64) << 128) - U256::from(1u64);
    let packed: U256 = ((additional_data & mask) << 128) | (balance & mask);
    B256::new(packed.to_be_bytes::<32>())
}

/// Probes whether `token` is an Aave v3 aToken and returns its `(pool,
/// underlying)` pair. Accepts the token iff the pool registers it as the
/// aToken for its declared underlying — rogue contracts implementing the
/// aToken selectors aren't enough.
pub async fn probe_aave_token(web3: &Web3, token: Address) -> Option<(Address, Address)> {
    let a_token = IAaveV3AToken::new(token, web3.provider.clone());
    let underlying_call = a_token.UNDERLYING_ASSET_ADDRESS();
    let pool_call = a_token.POOL();
    let (underlying, pool) = tokio::try_join!(underlying_call.call(), pool_call.call()).ok()?;
    let reserve = IAaveV3Pool::new(pool, web3.provider.clone())
        .getReserveData(underlying)
        .call()
        .await
        .ok()?;
    (reserve.aTokenAddress == token).then_some((pool, underlying))
}

/// Builds a state override that makes `balanceOf(holder)` on the aToken
/// report approximately `amount`. Writes into the canonical `_userState`
/// slot (`USER_STATE_SLOT`) shared by all Aave v3 aTokens. Returns `None`
/// if we can't reach the pool or the math overflows.
pub async fn build_override(
    web3: &Web3,
    a_token: Address,
    pool: Address,
    underlying: Address,
    holder: Address,
    amount: U256,
) -> Option<(Address, AccountOverride)> {
    let Ok(index) = IAaveV3Pool::new(pool, web3.provider.clone())
        .getReserveNormalizedIncome(underlying)
        .call()
        .await
    else {
        tracing::warn!(
            ?pool,
            ?underlying,
            "failed to fetch Aave reserve normalized income"
        );
        return None;
    };

    let Some(scaled) = ray_div(amount, index) else {
        // Either `amount * RAY` overflowed U256 (only possible for an
        // astronomically large requested amount) or the pool returned a
        // zero index (never should happen for a live reserve). Either way,
        // surface it explicitly so we don't silently drop the override.
        tracing::warn!(
            ?a_token,
            %amount,
            %index,
            "ray_div overflow computing AaveV3AToken scaled balance"
        );
        return None;
    };
    let slot = mapping_slot_hash(&holder, &U256::from(USER_STATE_SLOT).to_be_bytes());
    let value = pack_user_state(scaled, U256::ZERO);

    tracing::trace!(
        ?a_token,
        ?holder,
        %amount,
        %index,
        %scaled,
        "computed AaveV3AToken balance override"
    );

    let state_override = AccountOverride {
        state_diff: Some(iter::once((slot, value)).collect()),
        ..Default::default()
    };
    Some((a_token, state_override))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::{address, hex},
        alloy_provider::mock::Asserter,
        alloy_sol_types::SolValue,
    };

    fn encode_address(addr: Address) -> String {
        hex::encode_prefixed(addr.into_word())
    }

    /// Encodes a `ReserveData` struct as the ABI return payload the pool
    /// would produce. All fields other than `aTokenAddress` are zero — we
    /// only care about that one for the probe.
    fn encode_reserve_data(a_token: Address) -> String {
        let data = ReserveData {
            configuration: ReserveConfigurationMap { data: U256::ZERO },
            liquidityIndex: 0,
            currentLiquidityRate: 0,
            variableBorrowIndex: 0,
            currentVariableBorrowRate: 0,
            currentStableBorrowRate: 0,
            lastUpdateTimestamp: alloy_primitives::Uint::ZERO,
            id: 0,
            aTokenAddress: a_token,
            stableDebtTokenAddress: Address::ZERO,
            variableDebtTokenAddress: Address::ZERO,
            interestRateStrategyAddress: Address::ZERO,
            accruedToTreasury: 0,
            unbacked: 0,
            isolationModeTotalDebt: 0,
        };
        hex::encode_prefixed(data.abi_encode())
    }

    /// Builds an `Asserter` primed with the three responses the probe
    /// expects, in order: `UNDERLYING_ASSET_ADDRESS()`, `POOL()`, and
    /// `getReserveData(underlying)`. `None` maps to a reverted call; `Some`
    /// maps to a success response containing the given value.
    fn probe_asserter(
        underlying: Option<Address>,
        pool: Option<Address>,
        reserve_a_token: Option<Address>,
    ) -> Asserter {
        let asserter = Asserter::new();
        match underlying {
            Some(u) => asserter.push_success(&encode_address(u)),
            None => asserter.push_failure_msg("execution reverted"),
        }
        match pool {
            Some(p) => asserter.push_success(&encode_address(p)),
            None => asserter.push_failure_msg("execution reverted"),
        }
        match reserve_a_token {
            Some(a) => asserter.push_success(&encode_reserve_data(a)),
            None => asserter.push_failure_msg("execution reverted"),
        }
        asserter
    }

    /// A contract that doesn't expose the aToken selectors — `balanceOf`
    /// throws when the probe calls `UNDERLYING_ASSET_ADDRESS()` — is
    /// cleanly rejected so non-aToken ERC-20s don't accidentally pick up
    /// the Aave strategy.
    #[tokio::test]
    async fn probe_aave_token_rejects_when_underlying_call_reverts() {
        let token = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let web3 = Web3::with_asserter(probe_asserter(None, None, None));
        assert_eq!(probe_aave_token(&web3, token).await, None);
    }

    /// The probe bails if the claimed pool doesn't respond to
    /// `getReserveData` — guards against a contract exposing both
    /// `UNDERLYING_ASSET_ADDRESS()` and `POOL()` while pointing at
    /// something that isn't actually an Aave v3 pool.
    #[tokio::test]
    async fn probe_aave_token_rejects_when_pool_is_not_aave() {
        let token = address!("1111111111111111111111111111111111111111");
        let pool = address!("2222222222222222222222222222222222222222");
        let underlying = address!("3333333333333333333333333333333333333333");
        let web3 = Web3::with_asserter(probe_asserter(Some(underlying), Some(pool), None));
        assert_eq!(probe_aave_token(&web3, token).await, None);
    }

    /// A rogue contract that impersonates the aToken interface and points
    /// at a real Aave pool is rejected: the pool registers a *different*
    /// `aTokenAddress` for the underlying, so the identity check fails.
    #[tokio::test]
    async fn probe_aave_token_rejects_when_pool_registers_a_different_atoken() {
        let rogue = address!("bad000000000000000000000000000000000cafe");
        let real_a_token = address!("4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8");
        let pool = address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2");
        let underlying = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let web3 = Web3::with_asserter(probe_asserter(
            Some(underlying),
            Some(pool),
            // Pool agrees on the pair but names the *real* aToken, not the rogue.
            Some(real_a_token),
        ));
        assert_eq!(probe_aave_token(&web3, rogue).await, None);
    }
}

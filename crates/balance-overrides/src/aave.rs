//! Helpers shared between the production balance-override path
//! (`BalanceOverrides`) and the auto-detector for Aave v3 aTokens.
//!
//! aTokens break the usual "balanceOf = storage[slot]" assumption twice:
//! - `balanceOf` returns `scaled_balance × getReserveNormalizedIncome / RAY`,
//!   not the raw slot value.
//! - Storage is packed `UserState { uint128 balance; uint128 additionalData }`
//!   in a single slot per holder.
//!
//! The helpers below encode exactly these two facts so both the override
//! builder and the detector probe/verify use the same math.

use {
    alloy_eips::BlockId,
    alloy_primitives::{Address, B256, Bytes, TxKind, U256, keccak256},
    alloy_provider::Provider,
    alloy_rpc_types::{TransactionInput, TransactionRequest, state::AccountOverride},
    alloy_sol_types::{SolCall, sol},
    ethrpc::Web3,
    std::iter,
};

sol! {
    /// Mirrors Aave v3's `DataTypes.ReserveConfigurationMap`.
    struct ReserveConfigurationMap {
        uint256 data;
    }

    /// Mirrors Aave v3's `DataTypes.ReserveData`. Only the `aTokenAddress`
    /// field is consumed here; the rest are present to keep the ABI layout
    /// exactly matched so decoding doesn't go off the rails.
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
    interface IAaveV3Pool {
        function getReserveNormalizedIncome(address asset) external view returns (uint256);
        function getReserveData(address asset) external view returns (ReserveData memory);
    }

    /// Minimal interface for an Aave v3 `AToken`; used by the detector to
    /// decide whether a token is an aToken without any hardcoded list.
    interface IAaveV3AToken {
        function UNDERLYING_ASSET_ADDRESS() external view returns (address);
        function POOL() external view returns (address);
    }
}

/// Ray (1e27) — Aave's 27-decimal fixed-point unit.
pub const RAY: U256 = U256::from_limbs([0x9fd0803ce8000000, 0x33b2e3c, 0, 0]);

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

/// Probes whether `token` is an Aave v3 aToken. The token self-declares its
/// `UNDERLYING_ASSET_ADDRESS()` and `POOL()`; we then ask the pool for the
/// registered `aTokenAddress` of that underlying and require it to equal the
/// probed token. This catches both "contract doesn't look anything like an
/// aToken" and "contract impersonates the aToken interface but is not
/// registered with the pool for its declared underlying."
pub async fn probe_a_token(web3: &Web3, token: Address) -> Option<(Address, Address)> {
    let underlying = call_address(
        web3,
        token,
        IAaveV3AToken::UNDERLYING_ASSET_ADDRESSCall {}.abi_encode(),
    )
    .await?;
    let pool = call_address(web3, token, IAaveV3AToken::POOLCall {}.abi_encode()).await?;
    let reserve = fetch_reserve_data(web3, pool, underlying).await?;
    (reserve.aTokenAddress == token).then_some((pool, underlying))
}

/// Fetches `getReserveData(underlying)` from an Aave v3 Pool.
async fn fetch_reserve_data(
    web3: &Web3,
    pool: Address,
    underlying: Address,
) -> Option<ReserveData> {
    let call = IAaveV3Pool::getReserveDataCall { asset: underlying };
    let calldata = Bytes::from(call.abi_encode());
    let tx = TransactionRequest {
        to: Some(TxKind::Call(pool)),
        input: TransactionInput::new(calldata),
        ..Default::default()
    };
    let bytes = web3.provider.call(tx).block(BlockId::latest()).await.ok()?;
    IAaveV3Pool::getReserveDataCall::abi_decode_returns(&bytes).ok()
}

/// Fetches `getReserveNormalizedIncome(underlying)` from an Aave v3 Pool.
pub async fn fetch_normalized_income(
    web3: &Web3,
    pool: Address,
    underlying: Address,
) -> Option<U256> {
    let call = IAaveV3Pool::getReserveNormalizedIncomeCall { asset: underlying };
    let calldata = Bytes::from(call.abi_encode());
    let tx = TransactionRequest {
        to: Some(TxKind::Call(pool)),
        input: TransactionInput::new(calldata),
        ..Default::default()
    };
    let bytes = web3.provider.call(tx).block(BlockId::latest()).await.ok()?;
    IAaveV3Pool::getReserveNormalizedIncomeCall::abi_decode_returns(&bytes).ok()
}

/// Builds a state override that makes `balanceOf(holder)` on the aToken
/// report approximately `amount`. Returns `None` if we can't reach the pool
/// or the math overflows.
pub async fn build_override(
    web3: &Web3,
    a_token: Address,
    pool: Address,
    underlying: Address,
    map_slot: U256,
    holder: Address,
    amount: U256,
) -> Option<(Address, AccountOverride)> {
    let index = match fetch_normalized_income(web3, pool, underlying).await {
        Some(index) => index,
        None => {
            tracing::warn!(
                ?pool,
                ?underlying,
                "failed to fetch Aave reserve normalized income"
            );
            return None;
        }
    };

    let scaled = ray_div(amount, index)?;
    let slot = mapping_slot_hash(&holder, &map_slot.to_be_bytes::<32>());
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

/// Helper: `eth_call` a zero-argument view returning `address`. Returns
/// `None` on any RPC error or decode failure (including the common case of
/// the token not exposing this selector at all).
async fn call_address(web3: &Web3, to: Address, calldata: Vec<u8>) -> Option<Address> {
    let tx = TransactionRequest {
        to: Some(TxKind::Call(to)),
        input: TransactionInput::new(calldata.into()),
        ..Default::default()
    };
    let bytes = web3.provider.call(tx).block(BlockId::latest()).await.ok()?;
    if bytes.len() < 32 {
        return None;
    }
    // ABI-encoded `address` is right-aligned in the 32-byte word.
    let addr = Address::from_slice(&bytes[12..32]);
    // Treat zero as "not an aToken" — a zero underlying/pool is never legal.
    (!addr.is_zero()).then_some(addr)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::address,
        alloy_provider::mock::Asserter,
        alloy_sol_types::SolValue,
    };

    fn encode_address(addr: Address) -> String {
        format!("0x{:0>64x}", U256::from_be_bytes(addr.into_word().0))
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
        format!("0x{}", alloy_primitives::hex::encode(data.abi_encode()))
    }

    /// The probe returns `Some((pool, underlying))` when the token exposes
    /// both selectors and the pool confirms the token as the registered
    /// aToken for that underlying.
    #[tokio::test]
    async fn probe_a_token_accepts_valid_atoken() {
        let a_token = address!("4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8");
        let pool = address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2");
        let underlying = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");

        let asserter = Asserter::new();
        // 1. UNDERLYING_ASSET_ADDRESS() → underlying
        asserter.push_success(&encode_address(underlying));
        // 2. POOL() → pool
        asserter.push_success(&encode_address(pool));
        // 3. pool.getReserveData(underlying) → ReserveData { aTokenAddress: a_token, ..
        //    }
        asserter.push_success(&encode_reserve_data(a_token));

        let web3 = Web3::with_asserter(asserter);
        assert_eq!(
            probe_a_token(&web3, a_token).await,
            Some((pool, underlying))
        );
    }

    /// Anything that doesn't answer both selectors is rejected, so non-aToken
    /// ERC-20s don't accidentally pick up the Aave strategy.
    #[tokio::test]
    async fn probe_a_token_rejects_when_underlying_call_reverts() {
        let token = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let asserter = Asserter::new();
        // First call (UNDERLYING_ASSET_ADDRESS) reverts → probe bails.
        asserter.push_failure_msg("execution reverted");
        let web3 = Web3::with_asserter(asserter);
        assert_eq!(probe_a_token(&web3, token).await, None);
    }

    /// The probe bails if the pool doesn't look like an Aave v3 Pool
    /// (e.g. `getReserveData` reverts). This guards against a false
    /// positive where some random contract exposes both
    /// `UNDERLYING_ASSET_ADDRESS()` and `POOL()` but the named pool has
    /// nothing to do with Aave.
    #[tokio::test]
    async fn probe_a_token_rejects_when_pool_is_not_aave() {
        let token = address!("1111111111111111111111111111111111111111");
        let pool = address!("2222222222222222222222222222222222222222");
        let underlying = address!("3333333333333333333333333333333333333333");

        let asserter = Asserter::new();
        asserter.push_success(&encode_address(underlying));
        asserter.push_success(&encode_address(pool));
        asserter.push_failure_msg("execution reverted");

        let web3 = Web3::with_asserter(asserter);
        assert_eq!(probe_a_token(&web3, token).await, None);
    }

    /// A rogue contract that impersonates the aToken interface and points at
    /// a real Aave pool is rejected: the pool registers a *different*
    /// `aTokenAddress` for the underlying, so the identity check fails.
    #[tokio::test]
    async fn probe_a_token_rejects_when_pool_registers_a_different_atoken() {
        let rogue = address!("bad000000000000000000000000000000000cafe");
        let real_a_token = address!("4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8");
        let pool = address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2");
        let underlying = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");

        let asserter = Asserter::new();
        asserter.push_success(&encode_address(underlying));
        asserter.push_success(&encode_address(pool));
        // Pool agrees on the pair but names the *real* aToken, not the rogue.
        asserter.push_success(&encode_reserve_data(real_a_token));

        let web3 = Web3::with_asserter(asserter);
        assert_eq!(probe_a_token(&web3, rogue).await, None);
    }
}

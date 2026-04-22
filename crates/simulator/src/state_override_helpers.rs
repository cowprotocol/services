use {
    alloy_primitives::{Address, B256, Bytes, U256, keccak256},
    alloy_rpc_types::state::AccountOverride,
    std::iter,
};
pub use {
    balance_overrides::{BalanceOverrideRequest, BalanceOverrides, BalanceOverriding},
    configs::balance_overrides::Strategy,
};

/// Deploys a fake ERC-1271 contract at a given address so that signature
/// verification succeeds unconditionally. Pass to
/// [`crate::simulation_builder::SimulationBuilder::state_override`] with the
/// order owner's address.
pub struct FakeUser;

impl From<FakeUser> for AccountOverride {
    fn from(_fake_user: FakeUser) -> Self {
        let code = Bytes::from_static(&[
            0x63, 0x16, 0x26, 0xba, 0x7e, // PUSH4 0x1626ba7e
            0x60, 0xe0, // PUSH1 224
            0x1b, // SHL → 0x1626ba7e left-aligned in 32-byte word
            0x60, 0x00, // PUSH1 0x00
            0x52, // MSTORE
            0x60, 0x20, // PUSH1 0x20 (return 32 bytes)
            0x60, 0x00, // PUSH1 0x00 (from offset 0)
            0xf3, // RETURN
        ]);
        Self {
            code: Some(code),
            ..Default::default()
        }
    }
}

/// Sets the ETH balance of an address to the given value.
pub struct EthBalanceOverride(pub U256);

impl From<EthBalanceOverride> for AccountOverride {
    fn from(EthBalanceOverride(balance): EthBalanceOverride) -> Self {
        Self {
            balance: Some(balance),
            ..Default::default()
        }
    }
}

/// Overrides the authenticator contract's storage to allowlist a single solver
/// address. Pass to
/// [`crate::simulation_builder::SimulationBuilder::state_override`]
/// with the authenticator contract's address.
pub struct SolverAllowlisting(pub Address);

impl From<SolverAllowlisting> for AccountOverride {
    fn from(SolverAllowlisting(solver): SolverAllowlisting) -> Self {
        // GPv2AllowListAuthentication stores `mapping(address => bool) managers`
        // at storage slot 1. Solidity mapping key: keccak256(address_padded ++
        // slot_padded).
        let mut buf = [0u8; 64];
        buf[12..32].copy_from_slice(solver.as_slice());
        buf[32..64].copy_from_slice(&U256::ONE.to_be_bytes::<32>());
        let slot = keccak256(buf);
        Self {
            state_diff: Some(iter::once((slot, B256::with_last_byte(1))).collect()),
            ..Default::default()
        }
    }
}

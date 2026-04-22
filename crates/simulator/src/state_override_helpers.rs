use {alloy_primitives::Bytes, alloy_rpc_types::state::AccountOverride};
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

/// Deploys the `AnyoneAuthenticator` contract at a given address, causing it
/// to approve any solver. Pass to
/// [`crate::simulation_builder::SimulationBuilder::state_override`]
/// with the authenticator contract's address.
pub struct AnyoneAuthenticator;

impl From<AnyoneAuthenticator> for AccountOverride {
    fn from(_: AnyoneAuthenticator) -> Self {
        Self {
            code: Some(
                contracts::support::AnyoneAuthenticator::AnyoneAuthenticator::DEPLOYED_BYTECODE
                    .clone(),
            ),
            ..Default::default()
        }
    }
}

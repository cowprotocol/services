use bitflags::bitflags;

bitflags! {
    /// Validation phases that can be selectively executed
    ///
    /// The validation system is split into two distinct phases:
    /// - **INTRINSIC**: Checks that don't depend on blockchain state. These are synchronous.
    /// - **EXTRINSIC**: Checks that require blockchain state. These are asynchronous.
    ///
    /// Clients can select which phases to run using bitflags:
    /// - `ValidationPhases::INTRINSIC` - Only intrinsic checks (fast path for quotes)
    /// - `ValidationPhases::EXTRINSIC` - Only extrinsic checks (rare)
    /// - `ValidationPhases::ALL` - Both phases (full validation)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ValidationPhases: u8 {
        /// Intrinsic checks: No blockchain state required (SYNCHRONOUS)
        ///
        /// Intrinsic checks validate order properties that don't depend on external state:
        /// - Signature format validation (not EIP-1271 on-chain verification)
        /// - Validity period (valid_to timestamp)
        /// - Same buy/sell token check
        /// - Native token restrictions
        /// - Order type support
        /// - Token destination/source support
        /// - Zero amount checks
        /// - Fee sign checks
        /// - Owner recovery from ECDSA/EthSign signatures
        /// - App data validation (JSON parsing)
        const INTRINSIC = 0b00000001;

        /// Extrinsic checks: Requires blockchain state (ASYNC)
        ///
        /// Extrinsic checks require blockchain state or external calls:
        /// - EIP-1271 signature verification (on-chain contract call)
        /// - Balance checks (blockchain state)
        /// - Allowance checks (blockchain state)
        /// - Transfer simulation (state override)
        /// - Bad token detection (on-chain queries)
        /// - Quote calculation (price estimation)
        /// - Gas estimation
        /// - Limit order counting (database queries)
        /// - Banned user checking (may use on-chain Chainalysis oracle)
        /// - Wrapper contract validation (on-chain call)
        const EXTRINSIC = 0b00000010;
    }
}

impl ValidationPhases {
    /// Both intrinsic and extrinsic validation
    pub const ALL: Self = Self::from_bits_truncate(Self::INTRINSIC.bits() | Self::EXTRINSIC.bits());

    /// Check if intrinsic validation is included
    pub fn includes_intrinsic(self) -> bool {
        self.contains(Self::INTRINSIC)
    }

    /// Check if extrinsic validation is included
    pub fn includes_extrinsic(self) -> bool {
        self.contains(Self::EXTRINSIC)
    }
}

impl Default for ValidationPhases {
    fn default() -> Self {
        Self::ALL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_phases_bitflags() {
        assert!(ValidationPhases::ALL.includes_intrinsic());
        assert!(ValidationPhases::ALL.includes_extrinsic());

        let intrinsic_only = ValidationPhases::INTRINSIC;
        assert!(intrinsic_only.includes_intrinsic());
        assert!(!intrinsic_only.includes_extrinsic());

        let extrinsic_only = ValidationPhases::EXTRINSIC;
        assert!(!extrinsic_only.includes_intrinsic());
        assert!(extrinsic_only.includes_extrinsic());
    }

    #[test]
    fn validation_phases_default_is_all() {
        let default = ValidationPhases::default();
        assert_eq!(default, ValidationPhases::ALL);
    }
}

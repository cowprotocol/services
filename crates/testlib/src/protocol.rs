//! Mainnet addresses of protocol contracts.

use ethcontract::H160;

/// Address for the settlement contract.
pub const SETTLEMENT: H160 = H160(hex_literal::hex!(
    "9008D19f58AAbD9eD0D60971565AA8510560ab41"
));

/// Address for the vault relayer contract.
pub const RELAYER: H160 = H160(hex_literal::hex!(
    "C92E8bdf79f0507f65a392b0ab4667716BFE0110"
));

/// Address for the settlement contract.
pub const AUTHENTICATOR: H160 = H160(hex_literal::hex!(
    "2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"
));

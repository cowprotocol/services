use alloy::primitives::{U256, utils::Unit};

pub trait EthUnit: std::marker::Sized {
    /// Converts this value to wei.
    fn atom(self) -> U256;

    /// Converts this value from Mwei to wei (multiplies by 1e6).
    fn matom(self) -> U256 {
        self.atom() * U256::from(10).pow(U256::from(6))
    }

    /// Converts this value from Gwei to wei (multiplies by 1e9).
    fn gatom(self) -> U256 {
        self.atom() * U256::from(10).pow(U256::from(9))
    }

    /// Converts this value from Eth to wei (multiplies by 1e18).
    fn eth(self) -> U256 {
        self.atom() * Unit::ETHER.wei()
    }
}

impl EthUnit for u64 {
    fn atom(self) -> U256 {
        U256::from(self)
    }
}

impl EthUnit for u128 {
    fn atom(self) -> U256 {
        U256::from(self)
    }
}
impl EthUnit for f64 {
    fn atom(self) -> U256 {
        U256::from(self as u128)
    }

    fn eth(self) -> U256 {
        U256::from((self * 1e18) as u128)
    }
}

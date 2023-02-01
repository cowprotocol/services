pub use ethereum_types::{H160, U256};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenAddress(pub H160);

impl Display for TokenAddress {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str("0x")?;
        for b in self.0.as_bytes() {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for TokenAddress {
    type Err = <H160 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

/// The WETH token (or equivalent) for the EVM compatible network.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WethAddress(pub H160);

impl Display for WethAddress {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", TokenAddress(self.0))
    }
}

impl FromStr for WethAddress {
    type Err = <H160 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    pub amount: U256,
    pub token: TokenAddress,
}

/// Gas amount.
#[derive(Debug, Default, Clone, Copy)]
pub struct Gas(pub U256);

/// A 256-bit rational type.
pub type Rational = num::rational::Ratio<U256>;

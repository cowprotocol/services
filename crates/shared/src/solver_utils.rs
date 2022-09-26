use anyhow::{ensure, Result};
use serde::{
    de::{Deserializer, Error as _},
    Deserialize,
};
use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};

/// A slippage amount.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Default)]
pub struct Slippage(pub f64);

impl Slippage {
    pub const ONE_PERCENT: Self = Self(1.);

    /// Creates a slippage amount from the specified percentage.
    pub fn percentage(amount: f64) -> Result<Self> {
        // 1Inch API only accepts a slippage from 0 to 50.
        ensure!(
            (0. ..=50.).contains(&amount),
            "slippage outside of [0%, 50%] range"
        );
        Ok(Slippage(amount))
    }

    /// Creates a slippage amount from the specified basis points.
    pub fn percentage_from_basis_points(basis_points: u32) -> Result<Self> {
        let percent = (basis_points as f64) / 100.;
        Slippage::percentage(percent)
    }

    /// Creates a slippage amount from the specified basis points as number.
    pub fn number_from_basis_points(basis_points: u32) -> Result<Self> {
        let number_representation = (basis_points as f64) / 10000.;
        Ok(Slippage(number_representation))
    }
}

impl Display for Slippage {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn deserialize_decimal_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let decimal_str = Cow::<str>::deserialize(deserializer)?;
    decimal_str.parse::<f64>().map_err(D::Error::custom)
}

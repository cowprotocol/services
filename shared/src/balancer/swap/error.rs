//! Module listing all the errors from the Balancer contracts that are needed
//! for this project. An exhaustive list can be found here:
//! https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/solidity-utils/contracts/helpers/BalancerErrors.sol

use std::fmt;

macro_rules! errors_from_codes {
    ( $( ( $variant:ident, $code:literal ) ),+ $(,)? ) => {
        #[derive(thiserror::Error, Debug, PartialEq, Eq)]
        pub enum Error {
            $(
                $variant,
            )*
        }

        impl fmt::Display for Error {
            fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                match self {
                    $(
                        Self::$variant => write!(f, "{}", format!("BAL#{:0>3}: {}", $code, stringify!($variant))),
                    )*
                }
            }
        }

        #[cfg(test)]
        impl From<&str> for Error {
            fn from(errno: &str) -> Self {
                match errno.parse::<u16>().unwrap() {
                    $(
                        $code => Self::$variant,
                    )*
                    _ => panic!("Invalid error code"),
                }
            }
        }
    }
}

errors_from_codes!(
    (AddOverflow, 0),
    (SubOverflow, 1),
    (MulOverflow, 3),
    (ZeroDivision, 4),
    (DivInternal, 5),
    (XOutOfBounds, 6),
    (YOutOfBounds, 7),
    (ProductOutOfBounds, 8),
    (InvalidExponent, 9),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_error_formatting() {
        assert_eq!(format!("{}", Error::XOutOfBounds), "BAL#006: XOutOfBounds");
    }
}

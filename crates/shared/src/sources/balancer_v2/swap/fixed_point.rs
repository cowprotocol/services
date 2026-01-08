//! Module emulating the operations on fixed points with exactly 18 decimals as
//! used in the Balancer smart contracts. Their original implementation can be
//! found at:
//! https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/solidity-utils/contracts/math/FixedPoint.sol

use {
    super::error::Error,
    alloy::primitives::U256,
    anyhow::{Context, Result, bail, ensure},
    num::{BigInt, BigRational},
    number::conversions::{big_int_to_u256, u256_to_big_int},
    std::{
        convert::TryFrom,
        fmt::{self, Debug, Formatter},
        str::FromStr,
        sync::LazyLock,
    },
};

mod logexpmath;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
/// Fixed point numbers that represent exactly any rational number that can be
/// represented with up to 18 decimals as long as it can be stored in 256 bits.
/// It corresponds to Solidity's `ufixed256x18`.
/// Operations on this type are implemented as in Balancer's FixedPoint library,
/// including error codes, from which the name (Balancer Fixed Point).
pub struct Bfp(U256);

fn exp10(n: u8) -> U256 {
    U256::from(10u64).pow(U256::from(n))
}

static ONE_18: LazyLock<U256> = LazyLock::new(|| exp10(18));
static ONE_18_BIGINT: LazyLock<BigInt> = LazyLock::new(|| u256_to_big_int(&ONE_18));
static ZERO: LazyLock<Bfp> = LazyLock::new(|| Bfp(U256::ZERO));
static ONE: LazyLock<Bfp> = LazyLock::new(|| Bfp(*ONE_18));
static TWO: LazyLock<Bfp> = LazyLock::new(|| Bfp(*ONE_18 * U256::from(2u64)));
static FOUR: LazyLock<Bfp> = LazyLock::new(|| Bfp(*ONE_18 * U256::from(4u64)));
static MAX_POW_RELATIVE_ERROR: LazyLock<Bfp> = LazyLock::new(|| Bfp(U256::from(10000u64)));

impl From<usize> for Bfp {
    fn from(num: usize) -> Self {
        Self(U256::from(num).checked_mul(*ONE_18).unwrap())
    }
}

impl From<Bfp> for BigRational {
    fn from(num: Bfp) -> Self {
        BigRational::new(u256_to_big_int(&num.as_uint256()), u256_to_big_int(&ONE_18))
    }
}

impl<'a> TryFrom<&'a BigRational> for Bfp {
    type Error = anyhow::Error;

    fn try_from(value: &'a BigRational) -> Result<Self> {
        let scaled = value * &*ONE_18_BIGINT;
        ensure!(
            scaled.is_integer(),
            "remaining fractional component after scaling to fixed point"
        );
        Ok(Bfp(big_int_to_u256(&scaled.to_integer())?))
    }
}

impl FromStr for Bfp {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split_dot = s.splitn(2, '.');
        let units = split_dot
            .next()
            .expect("Splitting a string slice yields at least one element");
        let decimals = split_dot.next().unwrap_or("0");
        if units.is_empty() || decimals.is_empty() || decimals.len() > 18 {
            bail!("Invalid decimal representation");
        }
        Ok(Bfp(U256::from_str_radix(&format!("{decimals:0<18}"), 10)?
            .checked_add(
                U256::from_str_radix(units, 10)?
                    .checked_mul(*ONE_18)
                    .context("Too large number")?,
            )
            .context("Too large number")?))
    }
}

impl Debug for Bfp {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(
            formatter,
            "{}.{:0>18}",
            self.0 / *ONE_18,
            u128::try_from(self.0 % *ONE_18).unwrap()
        )
    }
}

impl Bfp {
    #[cfg(test)]
    pub fn to_f64_lossy(self) -> f64 {
        f64::from(self.as_uint256()) / 1e18
    }

    pub fn as_uint256(self) -> U256 {
        self.0
    }

    pub fn zero() -> Self {
        *ZERO
    }

    pub fn one() -> Self {
        *ONE
    }

    /// Returns 10 to the power of `exp` as a fixed point number.
    ///
    /// Note that this implementation truncates for exponents less than the
    /// smallest representable value (i.e. when `exp < -18`).
    pub fn exp10(exp: i32) -> Self {
        let exp = exp.saturating_add(18);
        if exp < 0 {
            return Self::zero();
        }

        Self(U256::from(10).pow(U256::from(exp)))
    }

    pub fn from_wei(num: U256) -> Self {
        Self(num)
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[expect(clippy::should_implement_trait)]
    pub fn add(self, other: Self) -> Result<Self, Error> {
        Ok(Self(self.0.checked_add(other.0).ok_or(Error::AddOverflow)?))
    }

    #[expect(clippy::should_implement_trait)]
    pub fn sub(self, other: Self) -> Result<Self, Error> {
        Ok(Self(self.0.checked_sub(other.0).ok_or(Error::SubOverflow)?))
    }

    pub fn mul_down(self, other: Self) -> Result<Self, Error> {
        Ok(Self(
            self.0.checked_mul(other.0).ok_or(Error::MulOverflow)? / *ONE_18,
        ))
    }

    pub fn mul_up(self, other: Self) -> Result<Self, Error> {
        let product = self.0.checked_mul(other.0).ok_or(Error::MulOverflow)?;

        Ok(if product.is_zero() {
            Bfp::zero()
        } else {
            Bfp(((product - U256::ONE) / *ONE_18) + U256::ONE)
        })
    }

    pub fn div_down(self, other: Self) -> Result<Self, Error> {
        if other.is_zero() {
            Err(Error::ZeroDivision)
        } else {
            Ok(Self(
                self.0.checked_mul(*ONE_18).ok_or(Error::DivInternal)? / other.0,
            ))
        }
    }

    pub fn div_up(self, other: Self) -> Result<Self, Error> {
        if other.is_zero() {
            return Err(Error::ZeroDivision);
        }
        if self.is_zero() {
            Ok(Self::zero())
        } else {
            let a_inflated = self.0.checked_mul(*ONE_18).ok_or(Error::DivInternal)?;

            Ok(Self(((a_inflated - U256::ONE) / other.0) + U256::ONE))
        }
    }

    pub fn complement(self) -> Self {
        if self.0 < *ONE_18 {
            Self(*ONE_18 - self.0)
        } else {
            Self::zero()
        }
    }

    pub fn pow_up(self, exp: Self) -> Result<Self, Error> {
        let raw = Bfp(logexpmath::pow(self.0, exp.0)?);
        let max_error = raw.mul_up(*MAX_POW_RELATIVE_ERROR)?.add(Bfp(U256::ONE))?;

        raw.add(max_error)
    }

    pub fn pow_up_v3(self, exp: Self) -> Result<Self, Error> {
        if exp == *ONE {
            Ok(self)
        } else if exp == *TWO {
            self.mul_up(self)
        } else if exp == *FOUR {
            let square = self.mul_up(self)?;
            square.mul_up(square)
        } else {
            let raw = Bfp(logexpmath::pow(self.0, exp.0)?);
            let max_error = raw.mul_up(*MAX_POW_RELATIVE_ERROR)?.add(Bfp(U256::ONE))?;

            raw.add(max_error)
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        num::{BigInt, One, Zero},
    };

    static EPSILON: LazyLock<Bfp> = LazyLock::new(|| Bfp(U256::ONE));

    fn test_exp10(n: u8) -> U256 {
        U256::from(10u64).pow(U256::from(n))
    }

    #[test]
    fn parsing() {
        assert_eq!("1".parse::<Bfp>().unwrap(), Bfp::one());
        assert_eq!("0.1".parse::<Bfp>().unwrap(), Bfp::from_wei(test_exp10(17)));
        assert_eq!(
            "1.01".parse::<Bfp>().unwrap(),
            Bfp::from_wei(test_exp10(18) + test_exp10(16))
        );
        assert_eq!(
            "10.000000000000000001".parse::<Bfp>().unwrap(),
            Bfp::from_wei(test_exp10(19) + U256::ONE)
        );
        assert!("10.0000000000000000001".parse::<Bfp>().is_err());
        assert!("1.0.1".parse::<Bfp>().is_err());
        assert!(".1".parse::<Bfp>().is_err());
        assert!("1.".parse::<Bfp>().is_err());
        assert!("".parse::<Bfp>().is_err());
    }

    #[test]
    fn add() {
        assert_eq!(Bfp::from(40).add(Bfp::from(2)).unwrap(), Bfp::from(42));

        assert_eq!(
            Bfp(U256::MAX).add(*EPSILON).unwrap_err(),
            Error::AddOverflow
        );
    }

    #[test]
    fn sub() {
        assert_eq!(Bfp::from(50).sub(Bfp::from(8)).unwrap(), Bfp::from(42));

        assert_eq!(
            Bfp::one().sub(Bfp(*ONE_18 + U256::ONE)).unwrap_err(),
            Error::SubOverflow
        );
    }

    macro_rules! test_mul {
        ($fn_name:ident) => {
            assert_eq!(Bfp::from(6).$fn_name(Bfp::from(7)).unwrap(), Bfp::from(42));
            assert_eq!(Bfp::zero().$fn_name(Bfp::one()).unwrap(), Bfp::zero());
            assert_eq!(Bfp::one().$fn_name(Bfp::zero()).unwrap(), Bfp::zero());
            assert_eq!(
                Bfp::one().$fn_name(Bfp(U256::MAX / *ONE_18)).unwrap(),
                Bfp(U256::MAX / *ONE_18)
            );

            assert_eq!(
                Bfp::one()
                    .$fn_name(Bfp(U256::MAX / *ONE_18 + U256::ONE))
                    .unwrap_err(),
                Error::MulOverflow,
            );
        };
    }

    #[test]
    fn mul() {
        test_mul!(mul_down);
        test_mul!(mul_up);

        let one_half = Bfp(U256::from(5 * 10_u128.pow(17)));
        assert_eq!(EPSILON.mul_down(one_half).unwrap(), Bfp::zero());
        assert_eq!(EPSILON.mul_up(one_half).unwrap(), *EPSILON);

        // values used in proof:
        // shared/src/sources/balancer/swap/weighted_math.rs#L28-L33
        let max_in_ratio = Bfp::from_wei(test_exp10(17).checked_mul(U256::from(3u64)).unwrap());
        let balance_in = Bfp::from_wei(U256::MAX / (test_exp10(17) * U256::from(3u64)));
        assert!(balance_in.mul_down(max_in_ratio).is_ok());
        assert!(
            (balance_in.add(Bfp::one()))
                .unwrap()
                .mul_down(max_in_ratio)
                .is_err()
        );
    }

    macro_rules! test_div {
        ($fn_name:ident) => {
            assert_eq!(Bfp::from(42).div_down(Bfp::from(7)).unwrap(), Bfp::from(6));
            assert_eq!(Bfp::zero().div_down(Bfp::one()).unwrap(), Bfp::from(0));

            assert_eq!(
                Bfp::one().$fn_name(Bfp::zero()).unwrap_err(),
                Error::ZeroDivision
            );
            assert_eq!(
                Bfp(U256::MAX / *ONE_18 + U256::ONE)
                    .$fn_name(Bfp::one())
                    .unwrap_err(),
                Error::DivInternal,
            );
        };
    }

    #[test]
    fn div() {
        test_div!(div_down);
        test_div!(div_up);

        assert_eq!(EPSILON.div_down(Bfp::from(2)).unwrap(), Bfp::zero());
        assert_eq!(EPSILON.div_up(Bfp::from(2)).unwrap(), *EPSILON);
        assert_eq!(Bfp::zero().div_up(Bfp::from(1)).unwrap(), Bfp::zero());
    }

    #[test]
    fn pow_up() {
        assert_eq!(
            Bfp::from(2).pow_up(Bfp::from(3)).unwrap(),
            Bfp(U256::from(8_000_000_000_000_079_990_u128))
        ); // powDown: 7999999999999919988
        assert_eq!(
            Bfp::from(2).pow_up(Bfp::from(0)).unwrap(),
            Bfp(U256::from(1_000_000_000_000_010_001_u128))
        ); // powDown: 999999999999989999
        assert_eq!(Bfp::zero().pow_up(Bfp::one()).unwrap(), Bfp(U256::ONE)); // powDown: 0

        assert_eq!(
            Bfp(U256::MAX).pow_up(Bfp::one()).unwrap_err(),
            Error::XOutOfBounds,
        );
        // note: the values were chosen to get a large value from `pow`
        assert_eq!(
            Bfp(U256::from_str_radix(
                "287200000000000000000000000000000000000000000000000000000000000000000000000",
                10
            )
            .unwrap())
            .pow_up(Bfp::one())
            .unwrap_err(),
            Error::MulOverflow,
        );
    }

    #[test]
    fn complement() {
        assert_eq!(Bfp::zero().complement(), Bfp::one());
        assert_eq!(
            "0.424242424242424242".parse::<Bfp>().unwrap().complement(),
            "0.575757575757575758".parse().unwrap()
        );
        assert_eq!(Bfp::one().complement(), Bfp::zero());
        assert_eq!(
            "1.000000000000000001".parse::<Bfp>().unwrap().complement(),
            Bfp::zero()
        );
    }

    #[test]
    fn bfp_big_rational_round_trip() {
        let value = "0.5".parse::<Bfp>().unwrap();
        assert_eq!(Bfp::try_from(&BigRational::from(value)).unwrap(), value);
    }

    #[test]
    fn bfp_to_big_rational() {
        assert_eq!(BigRational::from(Bfp::zero()), BigRational::zero());
        assert_eq!(BigRational::from(Bfp::one()), BigRational::one());
        assert_eq!(
            BigRational::from(Bfp::from(500)),
            BigRational::new(BigInt::from(500), BigInt::one())
        );
        assert_eq!(
            BigRational::from(Bfp::from_wei(U256::MAX)),
            BigRational::new(
                BigInt::from_str(
                    "115792089237316195423570985008687907853269984665640564039457584007913129639935"
                )
                .unwrap(),
                BigInt::from(1_000_000_000_000_000_000u64)
            )
        );
        assert_eq!(
            BigRational::from("0.4".parse::<Bfp>().unwrap()),
            BigRational::new(BigInt::from(4), BigInt::from(10))
        );
        assert_eq!(
            BigRational::from("0.4".parse::<Bfp>().unwrap()),
            BigRational::new(BigInt::from(2), BigInt::from(5))
        );
    }

    #[test]
    fn big_rational_to_bfp() {
        assert_eq!(Bfp::try_from(&BigRational::zero()).unwrap(), Bfp::zero());
        assert_eq!(Bfp::try_from(&BigRational::one()).unwrap(), Bfp::one());
        assert_eq!(
            Bfp::try_from(&BigRational::new(500.into(), 1.into())).unwrap(),
            "500.0".parse().unwrap(),
        );
        assert_eq!(
            Bfp::try_from(&BigRational::new(4.into(), 10.into())).unwrap(),
            "0.4".parse().unwrap(),
        );
        assert_eq!(
            Bfp::try_from(&BigRational::new(
                u256_to_big_int(&U256::MAX),
                BigInt::from(1_000_000_000_000_000_000u64)
            ))
            .unwrap(),
            Bfp::from_wei(U256::MAX),
        );
    }

    #[test]
    fn big_rational_to_bfp_non_representable() {
        assert!(Bfp::try_from(&BigRational::new(2.into(), 3.into())).is_err());
    }

    #[test]
    fn big_rational_to_bfp_overflow() {
        assert!(
            Bfp::try_from(&BigRational::new(
                u256_to_big_int(&U256::MAX) + 1,
                BigInt::from(1_000_000_000_000_000_000u64)
            ))
            .is_err()
        );
    }

    #[test]
    fn bfp_from_string() {
        assert_eq!(
            Bfp::from_str(
                "999999999999999999999999999999999999999999999999999999999999999999999999999999"
            )
            .unwrap_err()
            .to_string(),
            "the value is too large to fit the target type"
        );
        assert_eq!(
            Bfp::from_str(
                "9999999999999999999999999999999999999999999999999999999999999999999999999"
            )
            .unwrap_err()
            .to_string(),
            "Too large number"
        );
        assert_eq!(
            Bfp::from_str(".").unwrap_err().to_string(),
            "Invalid decimal representation"
        );
    }

    #[test]
    fn bfp_debug() {
        assert_eq!(format!("{:?}", Bfp::one()), "1.000000000000000000");
    }

    #[test]
    fn bfp_exp10() {
        let exp10 = |exp: i32| format!("{:?}", Bfp::exp10(exp));

        assert_eq!(exp10(18), "1000000000000000000.000000000000000000");
        assert_eq!(exp10(6), "1000000.000000000000000000");
        assert_eq!(exp10(0), "1.000000000000000000");
        assert_eq!(exp10(-1), "0.100000000000000000");
        assert_eq!(exp10(-18), "0.000000000000000001");
        assert_eq!(exp10(-19), "0.000000000000000000");
        assert_eq!(exp10(-42), "0.000000000000000000");
    }
}

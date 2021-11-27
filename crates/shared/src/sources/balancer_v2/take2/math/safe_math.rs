use super::error::Error;
use ethcontract::U256;

pub trait BalSafeMath: Sized {
    fn bmul(self, other: Self) -> Result<Self, Error>;
    fn badd(self, other: Self) -> Result<Self, Error>;
    fn bsub(self, other: Self) -> Result<Self, Error>;
    fn bdiv_down(self, other: Self) -> Result<Self, Error>;
    fn bdiv_up(self, other: Self) -> Result<Self, Error>;
}

impl BalSafeMath for U256 {
    fn bmul(self, other: Self) -> Result<Self, Error> {
        self.checked_mul(other).ok_or(Error::MulOverflow)
    }

    fn badd(self, other: Self) -> Result<Self, Error> {
        self.checked_add(other).ok_or(Error::AddOverflow)
    }

    fn bsub(self, other: Self) -> Result<Self, Error> {
        self.checked_sub(other).ok_or(Error::SubOverflow)
    }

    fn bdiv_down(self, other: Self) -> Result<Self, Error> {
        if other.is_zero() {
            return Err(Error::ZeroDivision);
        }
        Ok(self / other)
    }

    fn bdiv_up(self, other: Self) -> Result<Self, Error> {
        if other.is_zero() {
            return Err(Error::ZeroDivision);
        }
        if self.is_zero() {
            return Ok(U256::zero());
        }
        let one = U256::one();
        Ok(one + (self - one) / other)
    }
}

pub fn rounded_div(numerator: U256, denominator: U256, round_up: bool) -> Result<U256, Error> {
    if round_up {
        numerator.bdiv_up(denominator)
    } else {
        numerator.bdiv_down(denominator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_div_ok() {
        assert_eq!(rounded_div(1.into(), 2.into(), true).unwrap(), 1.into());
        assert_eq!(rounded_div(1.into(), 2.into(), false).unwrap(), 0.into());
    }

    #[test]
    fn rounded_div_err() {
        assert_eq!(
            rounded_div(1.into(), 0.into(), true).unwrap_err(),
            Error::ZeroDivision,
        );
        assert_eq!(
            rounded_div(1.into(), 0.into(), false).unwrap_err(),
            Error::ZeroDivision,
        );
    }

    #[test]
    fn bmul_tests() {
        let zero = U256::zero();
        let one = U256::one();
        let max = U256::MAX;
        assert_eq!(zero.bmul(one).unwrap(), zero);
        assert_eq!(one.bmul(one).unwrap(), one);
        assert_eq!(max.bmul(one).unwrap(), max);
        assert_eq!(max.bmul(max).unwrap_err(), Error::MulOverflow);
    }

    #[test]
    fn badd_tests() {
        let zero = U256::zero();
        let one = U256::one();
        let two = U256::from(2);
        let max = U256::MAX;
        assert_eq!(zero.badd(one).unwrap(), one);
        assert_eq!(one.badd(one).unwrap(), two);
        assert_eq!(max.badd(zero).unwrap(), max);
        assert_eq!(max.badd(max).unwrap_err(), Error::AddOverflow);
    }

    #[test]
    fn bsub_tests() {
        let zero = U256::zero();
        let one = U256::one();
        let two = U256::from(2);
        assert_eq!(two.bsub(zero).unwrap(), two);
        assert_eq!(two.bsub(one).unwrap(), one);
        assert_eq!(two.bsub(two).unwrap(), zero);
        assert_eq!(one.bsub(two).unwrap_err(), Error::SubOverflow);
    }

    #[test]
    fn div_down_tests() {
        let zero = U256::zero();
        let one = U256::one();
        let two = U256::from(2);
        assert_eq!(zero.bdiv_down(one).unwrap(), zero);
        assert_eq!(two.bdiv_down(one).unwrap(), two);
        assert_eq!(two.bdiv_down(two).unwrap(), one);
        assert_eq!(one.bdiv_down(two).unwrap(), zero);
        assert_eq!(one.bdiv_down(zero).unwrap_err(), Error::ZeroDivision);
    }

    #[test]
    fn div_up_tests() {
        let zero = U256::zero();
        let one = U256::one();
        let two = U256::from(2);
        assert_eq!(zero.bdiv_up(one).unwrap(), zero);
        assert_eq!(two.bdiv_up(one).unwrap(), two);
        assert_eq!(two.bdiv_up(two).unwrap(), one);
        assert_eq!(one.bdiv_up(two).unwrap(), one);
        assert_eq!(one.bdiv_up(zero).unwrap_err(), Error::ZeroDivision);
    }
}

use {
    alloy::primitives::{U256, aliases::I512},
    anyhow::{Result, ensure},
    bigdecimal::{BigDecimal, num_bigint::ToBigInt},
    num::{BigInt, BigRational, BigUint, Zero, bigint::Sign, rational::Ratio},
};

pub fn big_decimal_to_big_uint(big_decimal: &BigDecimal) -> Option<BigUint> {
    // TODO(vkgnosis): It would be nice to avoid copying the underlying BigInt when
    // converting BigDecimal to anything else but the simple
    // big_decimal.to_bigint makes a copy internally.
    big_decimal.to_bigint()?.try_into().ok()
}

pub fn rational_to_big_decimal<T>(value: &Ratio<T>) -> BigDecimal
where
    T: Clone,
    BigInt: From<T>,
{
    let numer = BigInt::from(value.numer().clone());
    let denom = BigInt::from(value.denom().clone());
    BigDecimal::new(numer, 0) / BigDecimal::new(denom, 0)
}

pub fn big_decimal_to_big_rational(value: &BigDecimal) -> BigRational {
    let (numer, scale) = value.as_bigint_and_exponent();
    let (adjusted_numer, denom) = match scale.cmp(&0) {
        std::cmp::Ordering::Equal => (numer, BigInt::from(1)),
        std::cmp::Ordering::Greater => (numer, BigInt::from(10).pow(scale as u32)),
        std::cmp::Ordering::Less => (
            numer * BigInt::from(10).pow((-scale) as u32),
            BigInt::from(1),
        ),
    };

    BigRational::new(adjusted_numer, denom)
}

pub fn big_uint_to_u256(input: &BigUint) -> Result<U256> {
    let bytes = input.to_bytes_be();
    ensure!(bytes.len() <= 32, "too large");
    Ok(U256::from_be_slice(&bytes))
}

pub fn big_int_to_u256(input: &BigInt) -> Result<U256> {
    ensure!(input.sign() != Sign::Minus, "negative");
    big_uint_to_u256(input.magnitude())
}

pub fn big_decimal_to_u256(big_decimal: &BigDecimal) -> Option<U256> {
    if !big_decimal.is_integer() {
        return None;
    }
    let big_int = big_decimal.to_bigint()?;
    big_int_to_u256(&big_int).ok()
}

pub fn big_rational_to_u256(ratio: &BigRational) -> Result<U256> {
    ensure!(!ratio.denom().is_zero(), "zero denominator");
    big_int_to_u256(&(ratio.numer() / ratio.denom()))
}

pub fn u256_to_big_uint(input: &U256) -> BigUint {
    BigUint::from_bytes_be(&input.to_be_bytes::<32>())
}

pub fn u256_to_big_int(input: &U256) -> BigInt {
    BigInt::from_biguint(Sign::Plus, u256_to_big_uint(input))
}

pub fn u256_to_big_rational(input: &U256) -> BigRational {
    BigRational::new(u256_to_big_int(input), 1.into())
}

pub fn u256_to_big_decimal(u256: &U256) -> BigDecimal {
    let big_uint = u256_to_big_uint(u256);
    BigDecimal::from(BigInt::from(big_uint))
}

pub fn i512_to_big_int(i512: &I512) -> BigInt {
    BigInt::from_bytes_be(
        match i512.sign() {
            alloy::primitives::Sign::Positive => Sign::Plus,
            alloy::primitives::Sign::Negative => Sign::Minus,
        },
        &i512.abs().to_be_bytes::<64>(),
    )
}

pub fn i512_to_big_rational(input: &I512) -> BigRational {
    BigRational::new(i512_to_big_int(input), 1.into())
}

// TODO: Figure out a nicer way to convert I512 to U256, as a follow-up task
pub fn i512_to_u256(input: &I512) -> Result<U256> {
    anyhow::ensure!(input >= &I512::ZERO, "Negative input value");
    anyhow::ensure!(input < &I512::from(U256::MAX), "Input exceeds U256::MAX");
    Ok(alloy::primitives::U256::from_be_slice(
        &input.to_be_bytes::<64>()[32..],
    ))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::conversions::{
            big_decimal_to_big_rational,
            big_decimal_to_big_uint,
            rational_to_big_decimal,
        },
        num::{One, rational::Ratio},
        std::str::FromStr,
    };

    #[test]
    fn big_integer_to_u256() {
        for val in &[0i32, 42, 1337] {
            assert_eq!(
                big_int_to_u256(&BigInt::from(*val)).unwrap(),
                U256::from(*val),
            );
        }
    }

    #[test]
    fn rational_to_big_decimal_() {
        let v = Ratio::new(3u16, 1_000u16);
        let c = rational_to_big_decimal(&v);
        assert_eq!(c, BigDecimal::new(3.into(), 3));
    }

    #[test]
    fn big_decimal_to_big_rational_() {
        let v = BigDecimal::from_str("1234567890.0987654321234567890").unwrap();
        let c = big_decimal_to_big_rational(&v);
        assert_eq!(
            c,
            BigRational::new(
                BigInt::from(1234567890098765432123456789u128),
                BigInt::from(1000000000000000000u64)
            )
        );

        let v = BigDecimal::from_str("0.0987654321234567890").unwrap();
        let c = big_decimal_to_big_rational(&v);
        assert_eq!(
            c,
            BigRational::new(
                BigInt::from(98765432123456789u64),
                BigInt::from(1000000000000000000u64)
            )
        );

        let v = BigDecimal::from_str("1e-19").unwrap();
        let c = big_decimal_to_big_rational(&v);
        assert_eq!(
            c,
            BigRational::new(BigInt::from(1), BigInt::from(10000000000000000000u64))
        );

        let v = BigDecimal::new(BigInt::from(1000000), -4);
        let c = big_decimal_to_big_rational(&v);
        assert_eq!(
            c,
            BigRational::new(BigInt::from(10000000000u64), BigInt::from(1))
        );
    }

    #[test]
    fn u256_to_big_uint_() {
        assert_eq!(u256_to_big_uint(&U256::ZERO), BigUint::zero());
        assert_eq!(u256_to_big_uint(&U256::ONE), BigUint::one());
        assert_eq!(
            u256_to_big_uint(&U256::MAX),
            BigUint::from_str(
                "115792089237316195423570985008687907853269984665640564039457584007913129639935"
            )
            .unwrap()
        );
    }

    #[test]
    fn bigint_to_u256_() {
        assert_eq!(big_int_to_u256(&BigInt::zero()).unwrap(), U256::ZERO);
        assert_eq!(big_int_to_u256(&BigInt::one()).unwrap(), U256::ONE);
        let max_u256_as_bigint = BigInt::from_str(
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        )
        .unwrap();
        assert_eq!(big_int_to_u256(&max_u256_as_bigint).unwrap(), U256::MAX);
        assert!(big_int_to_u256(&(max_u256_as_bigint + BigInt::one())).is_err());
        assert!(big_int_to_u256(&BigInt::from(-1)).is_err());
    }

    #[test]
    fn u256_to_big_decimal_() {
        assert_eq!(u256_to_big_decimal(&U256::ZERO), BigDecimal::zero());
        assert_eq!(u256_to_big_decimal(&U256::ONE), BigDecimal::one());
        assert_eq!(
            u256_to_big_decimal(&U256::MAX),
            BigDecimal::from_str(
                "115792089237316195423570985008687907853269984665640564039457584007913129639935"
            )
            .unwrap()
        );
    }

    #[test]
    fn big_decimal_to_big_uint_() {
        assert_eq!(
            big_decimal_to_big_uint(&BigDecimal::zero()),
            Some(BigUint::zero())
        );
        assert_eq!(
            big_decimal_to_big_uint(&BigDecimal::one()),
            Some(BigUint::one())
        );
        assert!(big_decimal_to_big_uint(
            &BigDecimal::from_str(

"9115792089237316195423570985008687907853269984665640564039457584007913129639935"
            )
            .unwrap()
        )
        .is_some());

        assert!(big_decimal_to_big_uint(&BigDecimal::from(-1)).is_none());
        assert!(big_decimal_to_u256(&BigDecimal::from_str("0.5").unwrap()).is_none());
    }

    #[test]
    fn big_decimal_to_u256_() {
        assert_eq!(big_decimal_to_u256(&BigDecimal::zero()), Some(U256::ZERO));
        assert_eq!(big_decimal_to_u256(&BigDecimal::one()), Some(U256::ONE));
        assert!(big_decimal_to_u256(&BigDecimal::from(-1)).is_none());
        assert!(big_decimal_to_u256(&BigDecimal::from_str("0.5").unwrap()).is_none());
        let max_u256_as_big_decimal = BigDecimal::from_str(
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        )
        .unwrap();
        assert_eq!(
            big_decimal_to_u256(&max_u256_as_big_decimal),
            Some(U256::MAX)
        );
        assert!(big_decimal_to_u256(&(max_u256_as_big_decimal + BigDecimal::one())).is_none());
    }
}

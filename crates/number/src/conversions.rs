use {
    anyhow::{ensure, Context, Result},
    bigdecimal::{num_bigint::ToBigInt, BigDecimal},
    num::{bigint::Sign, rational::Ratio, BigInt, BigRational, BigUint, Zero},
    primitive_types::U256,
    std::{ops::Neg, str::FromStr},
};

pub fn u256_to_big_uint(input: &U256) -> BigUint {
    let mut bytes = [0; 32];
    input.to_big_endian(&mut bytes);
    BigUint::from_bytes_be(&bytes)
}

pub fn u256_to_big_int(input: &U256) -> BigInt {
    BigInt::from_biguint(Sign::Plus, u256_to_big_uint(input))
}

pub fn u256_to_big_rational(input: &U256) -> BigRational {
    BigRational::new(u256_to_big_int(input), 1.into())
}

pub fn big_uint_to_u256(input: &BigUint) -> Result<U256> {
    let bytes = input.to_bytes_be();
    ensure!(bytes.len() <= 32, "too large");
    Ok(U256::from_big_endian(&bytes))
}

pub fn big_int_to_u256(input: &BigInt) -> Result<U256> {
    ensure!(input.sign() != Sign::Minus, "negative");
    big_uint_to_u256(input.magnitude())
}

pub fn big_rational_to_u256(ratio: &BigRational) -> Result<U256> {
    ensure!(!ratio.denom().is_zero(), "zero denominator");
    big_int_to_u256(&(ratio.numer() / ratio.denom()))
}

// TODO: It would be nice to avoid copying the underlying BigInt when converting
// BigDecimal to anything else but the simple big_decimal.to_bigint makes a copy
// internally.

pub fn u256_to_big_decimal(u256: &U256) -> BigDecimal {
    let big_uint = u256_to_big_uint(u256);
    BigDecimal::from(BigInt::from(big_uint))
}

pub fn big_decimal_to_big_uint(big_decimal: &BigDecimal) -> Option<BigUint> {
    big_decimal.to_bigint()?.try_into().ok()
}

pub fn big_decimal_to_u256(big_decimal: &BigDecimal) -> Option<U256> {
    if !big_decimal.is_integer() {
        return None;
    }
    let big_int = big_decimal.to_bigint()?;
    big_int_to_u256(&big_int).ok()
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

/// Floats in Rust have wrong bytes representation which leaks into
/// `BigRational` instance when converting from float.
///
/// This function converts a decimal string (e.g., `"0.1"`) to an exact
/// `BigRational`.
pub fn big_rational_from_decimal_str(s: &str) -> Result<BigRational> {
    let s = s.trim();
    // Handle negative numbers
    let (is_negative, s) = if let Some(stripped) = s.strip_prefix('-') {
        (true, stripped)
    } else {
        (false, s)
    };

    let parts: Vec<&str> = s.split('.').collect();
    match parts.len() {
        1 => {
            // No fractional part
            let numerator = BigInt::from_str(parts[0]).context("unable to parse integer part")?;
            Ok(BigRational::from_integer(numerator))
        }
        2 => {
            let integer_part = if parts[0].is_empty() {
                // Handle cases like ".5" as "0.5"
                BigInt::zero()
            } else {
                BigInt::from_str(parts[0]).context("unable to parse integer part")?
            };

            let fractional_part = if parts[1].is_empty() {
                // Handle cases like "1." as "1.0"
                BigInt::zero()
            } else {
                BigInt::from_str(parts[1]).context("unable to parse fractional part")?
            };

            let fractional_length = parts[1].len() as u32;

            let denominator = BigInt::from(10u32).pow(fractional_length);
            let numerator = integer_part
                .checked_mul(&denominator)
                .context("rational overflow during multiplication")?
                .checked_add(&fractional_part)
                .context("rational overflow during addition")?;
            Ok(BigRational::new(numerator, denominator))
        }
        _ => Err(anyhow::anyhow!("invalid decimal number")),
    }
    .map(|ratio| if is_negative { ratio.neg() } else { ratio })
}

#[cfg(test)]
mod tests {
    use {super::*, num::One, std::str::FromStr};

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
        assert_eq!(u256_to_big_uint(&U256::zero()), BigUint::zero());
        assert_eq!(u256_to_big_uint(&U256::one()), BigUint::one());
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
        assert_eq!(big_int_to_u256(&BigInt::zero()).unwrap(), U256::zero());
        assert_eq!(big_int_to_u256(&BigInt::one()).unwrap(), U256::one());
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
        assert_eq!(u256_to_big_decimal(&U256::zero()), BigDecimal::zero());
        assert_eq!(u256_to_big_decimal(&U256::one()), BigDecimal::one());
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
        assert_eq!(big_decimal_to_u256(&BigDecimal::zero()), Some(U256::zero()));
        assert_eq!(big_decimal_to_u256(&BigDecimal::one()), Some(U256::one()));
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

    #[test]
    fn big_rational_from_decimal_str_() {
        assert_eq!(
            big_rational_from_decimal_str("0").unwrap(),
            BigRational::zero()
        );
        assert_eq!(
            big_rational_from_decimal_str("1").unwrap(),
            BigRational::one()
        );
        assert_eq!(
            big_rational_from_decimal_str("-1").unwrap(),
            -BigRational::one()
        );
        assert_eq!(
            big_rational_from_decimal_str("1.0").unwrap(),
            BigRational::one()
        );
        assert_eq!(
            big_rational_from_decimal_str("1.").unwrap(),
            BigRational::one()
        );
        assert_eq!(
            big_rational_from_decimal_str("-1.").unwrap(),
            -BigRational::one()
        );
        assert_eq!(
            big_rational_from_decimal_str("1.000").unwrap(),
            BigRational::one()
        );
        assert_eq!(
            big_rational_from_decimal_str("0.1").unwrap(),
            BigRational::new(1.into(), 10.into())
        );
        assert_eq!(
            big_rational_from_decimal_str(".1").unwrap(),
            BigRational::new(1.into(), 10.into())
        );
        assert_eq!(
            big_rational_from_decimal_str("-.1").unwrap(),
            -BigRational::new(1.into(), 10.into())
        );
        assert_eq!(
            big_rational_from_decimal_str("0.125").unwrap(),
            BigRational::new(1.into(), 8.into())
        );
        assert_eq!(
            big_rational_from_decimal_str("-0.125").unwrap(),
            -BigRational::new(1.into(), 8.into())
        );

        assert!(big_rational_from_decimal_str("0.1.0").is_err());
        assert!(big_rational_from_decimal_str("a").is_err());
        assert!(big_rational_from_decimal_str("1 0").is_err());
    }
}

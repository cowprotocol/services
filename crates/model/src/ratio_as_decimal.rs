use bigdecimal::BigDecimal;
use num::{BigInt, BigRational};
use serde::{de, Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};
use std::{borrow::Cow, convert::TryInto, str::FromStr};

pub struct DecimalBigRational;

impl<'de> DeserializeAs<'de, BigRational> for DecimalBigRational {
    fn deserialize_as<D>(deserializer: D) -> Result<BigRational, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize(deserializer)
    }
}

impl SerializeAs<BigRational> for DecimalBigRational {
    fn serialize_as<S>(source: &BigRational, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize(source, serializer)
    }
}

pub fn serialize<S>(value: &BigRational, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let top_bytes = value.numer().to_bytes_le();
    let top = BigInt::from_bytes_le(top_bytes.0, &top_bytes.1);

    let bottom_bytes = value.denom().to_bytes_le();
    let bottom = BigInt::from_bytes_le(bottom_bytes.0, &bottom_bytes.1);
    let decimal = BigDecimal::from(top) / BigDecimal::from(bottom);
    serializer.serialize_str(&decimal.to_string())
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<BigRational, D::Error>
where
    D: Deserializer<'de>,
{
    let big_decimal =
        BigDecimal::from_str(&Cow::<str>::deserialize(deserializer)?).map_err(|err| {
            de::Error::custom(format!("failed to decode decimal BigDecimal: {}", err))
        })?;
    let (x, exp) = big_decimal.into_bigint_and_exponent();
    let numerator_bytes = x.to_bytes_le();
    let base = num::bigint::BigInt::from_bytes_le(numerator_bytes.0, &numerator_bytes.1);
    let ten = BigRational::new(10.into(), 1.into());
    let numerator = BigRational::new(base, 1.into());
    Ok(numerator
        / ten.pow(
            exp.try_into()
                .map_err(|err| de::Error::custom(format!("decimal exponent overflow: {}", err)))?,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use num::{BigRational, Zero};
    use serde_json::{json, value::Serializer};

    #[test]
    fn serializer() {
        assert_eq!(
            serialize(&BigRational::from_float(1.2).unwrap(), Serializer).unwrap(),
            json!("1.1999999999999999555910790149937383830547332763671875")
        );
        assert_eq!(
            serialize(
                &BigRational::new(1.into(), 3.into()),
                Serializer
            )
            .unwrap(),
            json!("0.3333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333")
        );
        assert_eq!(
            serialize(&BigRational::zero(), Serializer).unwrap(),
            json!("0")
        );
        assert_eq!(
            serialize(&BigRational::new((-1).into(), 1.into()), Serializer).unwrap(),
            json!("-1")
        );
    }

    #[test]
    fn deserialize_err() {
        assert!(deserialize(json!("hello")).is_err());
    }

    #[test]
    fn deserialize_ok() {
        assert_eq!(
            deserialize(json!("1.2")).unwrap(),
            BigRational::new(12.into(), 10.into())
        );
        assert_eq!(deserialize(json!("0")).unwrap(), BigRational::zero());
        assert_eq!(
            deserialize(json!("-1")).unwrap(),
            BigRational::new((-1).into(), 1.into())
        );
    }
}

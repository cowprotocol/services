use primitive_types::U256;
use serde::{de, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};
use std::fmt;

pub struct DecimalU256;

impl<'de> DeserializeAs<'de, U256> for DecimalU256 {
    fn deserialize_as<D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize(deserializer)
    }
}

impl SerializeAs<U256> for DecimalU256 {
    fn serialize_as<S>(source: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize(source, serializer)
    }
}

pub fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor {}
    impl<'de> de::Visitor<'de> for Visitor {
        type Value = U256;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a u256 encoded as a decimal encoded string")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            U256::from_dec_str(s).map_err(|err| {
                de::Error::custom(format!("failed to decode {:?} as decimal u256: {}", s, err))
            })
        }
    }

    deserializer.deserialize_str(Visitor {})
}

/// Converts an amount of units of an ERC20 token with the specified amount of
/// decimals into its decimal representation as a string.
///
/// # Examples
///
/// ```rust
/// use model::u256_decimal::format_units;
///
/// assert_eq!(format_units(42u64.into(), 0), "42");
/// assert_eq!(format_units(1_337_000u64.into(), 6), "1.337000")
/// ```
pub fn format_units(amount: U256, decimals: usize) -> String {
    let str_amount = amount.to_string();
    if decimals == 0 {
        str_amount
    } else if str_amount.len() <= decimals {
        format!("0.{:0>pad_left$}", str_amount, pad_left = decimals)
    } else {
        format!(
            "{}.{}",
            &str_amount[0..str_amount.len() - decimals],
            &str_amount[str_amount.len() - decimals..]
        )
    }
}

#[test]
fn test_format_units() {
    assert_eq!(format_units(1_337u64.into(), 0), "1337");
    assert_eq!(format_units(0u64.into(), 0), "0");
    assert_eq!(format_units(0u64.into(), 1), "0.0");
    assert_eq!(format_units(1u64.into(), 6), "0.000001");
    assert_eq!(format_units(999_999u64.into(), 6), "0.999999");
    assert_eq!(format_units(1_000_000u64.into(), 6), "1.000000");
    assert_eq!(format_units(1_337_000u64.into(), 6), "1.337000");
    assert_eq!(
        format_units(1_337_000_004_200u64.into(), 6),
        "1337000.004200"
    )
}

use hex_literal::hex;
use serde::{de, Deserializer, Serializer};
use serde_with::serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    fmt::{self, Debug, Formatter},
    str::FromStr,
};

/// Binary data which gets signed with an order and holds additional information about the order.
/// This format is extendable since there are many reserved/unused bits left.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct AppId(pub [u8; 32]);
// byte index increases from left to right
// bit index increases from left to right
// byte 0:
//    bit 0: isLiquidityOrder
//    bit 1-7: reserved
// byte 1-25: reserved
// byte 26-27: salt
// byte 28-31: partner_id

/// ID of partner which created the order and forwarded it to us.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq, Debug)]
pub struct PartnerId(pub [u8; 4]);

/// Arbitrary data to make otherwise identical orders unique.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq, Debug)]
pub struct Salt(pub [u8; 2]);

impl Debug for AppId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl FromStr for AppId {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s.strip_prefix("0x").unwrap_or(s), &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl Serialize for AppId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut bytes = [0u8; 2 + 32 * 2];
        bytes[..2].copy_from_slice(b"0x");
        // Can only fail if the buffer size does not match but we know it is correct.
        hex::encode_to_slice(self.0, &mut bytes[2..]).unwrap();
        // Hex encoding is always valid utf8.
        let s = std::str::from_utf8(&bytes).unwrap();
        serializer.serialize_str(s)
    }
}

impl<'de> Deserialize<'de> for AppId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        let value = s.parse().map_err(|err| {
            de::Error::custom(format!(
                "failed to decode {:?} as hex appdata 32 bytes: {}",
                s, err
            ))
        })?;
        Ok(value)
    }
}

impl AppId {
    // Those bit patterns have already been used by external integrators before it was well defined
    // what each bit in `AppId` was supposed to mean.
    // Until those integrators have been informed about the change and updated their implementation
    // we can't assume bits with special meaning have been set/unset on purpose.
    const EXCLUDED_PATTERNS: [[u8; 32]; 12] = [
        hex!("E9F29AE547955463ED535162AEFEE525D8D309571A2B18BC26086C8C35D781EB"),
        hex!("487B02C558D729ABAF3ECF17881A4181E5BC2446429A0995142297E897B6EB37"),
        hex!("BAB87DF726E41C0941786CA710194982D753FBC140C6DC9951AC3450D8917699"),
        hex!("E4D1AB10F2C9FFE7BDD23C315B03F18CFF90888D6B2BB5022BACD46AB9CDDF24"),
        hex!("A076A100C2535DC6047C4C9940AE647D7DEAAC1729745117D19D4A63BC2F4D30"),
        hex!("0BCAECDB9A1FDB3B207A7593EBF703836AD591D4E5E75DFDBF65E7B328F209CD"),
        hex!("828569F802B7F8957F76996BDD875674821E41A688541A9E9EC97D5E897D44A7"),
        hex!("A5DAE7A114F1BD6BB9B3FF976150380A95CB18856212DB555C25EF9D7801E9A4"),
        hex!("1C727C53F8A4552B7084DB0934A9A15C06DAA0EFF7878DE31A1A22D9ED4E6112"),
        hex!("f6a005bde820da47fdbb19bc07e56782b9ccec403a6899484cf502090627af8a"),
        hex!("00000000000000000000000055662e225a3376759c24331a9aed764f8f0c9fbb"),
        hex!("ACE3BC48303B96362EBCDEB46F2277C29716E832273B04C7EC528AF284061D54"),
    ];

    fn is_excluded(&self) -> bool {
        Self::EXCLUDED_PATTERNS.contains(&self.0)
    }

    /// Order should only be executed to enable trading user orders. It should not receive any
    /// surplus.
    pub fn is_liquidity_order(&self) -> bool {
        !self.is_excluded() && (self.0[0] & (1 << 7) != 0)
    }

    pub fn partner_id(&self) -> PartnerId {
        // We can interpret those bits even from excluded patterns because integrators already used
        // those bits for the partner id before the semantics of the AppId were well defined.
        let mut result = [0u8; 4];
        result.clone_from_slice(&self.0[28..]);
        PartnerId(result)
    }

    pub fn salt(&self) -> Salt {
        // We can interpret those bits even from excluded patterns because integrators used to use
        // all bits of the AppId for their partner id. This essentially already resulted in all of
        // their orders using the same salt we can't make the situation any worse for them.
        let mut result = [0u8; 2];
        result.clone_from_slice(&self.0[26..28]);
        Salt(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn works_on_32_byte_string_with_or_without_0x() {
        let with_0x = "0x0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83";
        let without_0x = "0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83";
        assert!(AppId::from_str(with_0x).is_ok());
        assert!(AppId::from_str(without_0x).is_ok());
        assert_eq!(AppId::from_str(with_0x), AppId::from_str(without_0x));
    }

    #[test]
    fn invalid_characters() {
        assert_eq!(
            AppId::from_str("xyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxy")
                .unwrap_err()
                .to_string(),
            "Invalid character 'x' at position 0"
        );
    }

    #[test]
    fn invalid_length() {
        assert_eq!(
            AppId::from_str("0x00").unwrap_err().to_string(),
            "Invalid string length"
        );
    }

    #[test]
    fn deserialize_app_id() {
        let value = json!("0x0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83");
        assert!(AppId::deserialize(value).is_ok());
        assert!(AppId::deserialize(json!("00")).is_err());
        assert!(AppId::deserialize(json!("asdf")).is_err());
        assert!(AppId::deserialize(json!("0x00")).is_err());
    }

    #[test]
    fn ignores_liquidity_flag_for_exluded_patterns() {
        for pattern in AppId::EXCLUDED_PATTERNS {
            let liquidity_bit_set = pattern[0] & (1u8 << 7) != 0;
            assert!(
                !AppId(pattern).is_liquidity_order(),
                "excluded patterns should always return the default value for the bit"
            );

            let mut excluded_pattern_with_purposeful_liquidity_bit = pattern;
            excluded_pattern_with_purposeful_liquidity_bit[0] ^= 1u8 << 7;
            assert_eq!(
                AppId(excluded_pattern_with_purposeful_liquidity_bit).is_liquidity_order(),
                !liquidity_bit_set,
                "an excluded pattern with the bit purposfully toggled should return the actual value of the bit"
            );
        }
    }

    #[test]
    fn set_and_unset_liquidity_flag() {
        let mut app_id = AppId::default();
        assert!(!app_id.is_liquidity_order());
        app_id.0[0] |= 1u8 << 7;
        assert!(app_id.is_liquidity_order());
    }

    #[test]
    fn read_partner_id() {
        let pattern = hex!("E9F29AE547955463ED535162AEFEE525D8D309571A2B18BC26086C8C35D781EB");
        let app_id = AppId(pattern);
        assert_eq!(app_id.partner_id(), PartnerId(hex!("35D781EB")));
    }

    #[test]
    fn read_salt() {
        let pattern = hex!("E9F29AE547955463ED535162AEFEE525D8D309571A2B18BC26086C8C35D781EB");
        let app_id = AppId(pattern);
        assert_eq!(app_id.salt(), Salt(hex!("6C8C")));
    }
}

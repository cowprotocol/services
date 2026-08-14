use {
    serde::{Deserialize, Deserializer},
    solana_sdk::pubkey::Pubkey,
};

/// Deserializes a base58-encoded Solana public key from a string.
pub fn deserialize_solana_pubkey_b58<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde::de::value::{Error, StrDeserializer},
    };

    #[test]
    fn deserializes_valid_base58_pubkey() {
        let de = StrDeserializer::<Error>::new("11111111111111111111111111111111");
        let result: Pubkey = deserialize_solana_pubkey_b58(de).unwrap();
        assert_eq!(result, Pubkey::default());
    }

    #[test]
    fn rejects_invalid_base58_pubkey() {
        let de = StrDeserializer::<Error>::new("not-a-valid-pubkey");
        let err: Error = deserialize_solana_pubkey_b58(de).unwrap_err();
        assert!(
            err.to_string().contains("Base58"),
            "unexpected error: {err}"
        );
    }
}

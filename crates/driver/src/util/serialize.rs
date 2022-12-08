use serde::{Deserialize, Serialize};

/// Serializes [`ethereum_types::U256`] as a decimal string.
#[derive(Debug, Serialize, Deserialize)]
pub struct U256(String);

impl From<ethereum_types::U256> for U256 {
    fn from(x: ethereum_types::U256) -> Self {
        Self(x.to_string())
    }
}

impl TryFrom<U256> for ethereum_types::U256 {
    type Error = &'static str;

    fn try_from(value: U256) -> Result<Self, Self::Error> {
        Self::from_dec_str(&value.0).map_err(|_| "invalid 256-bit decimal")
    }
}

/// Serializes binary data as a hexadecimal string.
#[derive(Debug, Serialize, Deserialize)]
pub struct Hex(String);

impl From<Vec<u8>> for Hex {
    fn from(data: Vec<u8>) -> Self {
        let hex = hex::encode(data);
        Self(format!("0x{hex}"))
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct U256(String);

// TODO Test that to_string and parse do what you expect here. It seems like
// they don't, they just convert to hex instead which is DUMB
impl From<ethereum_types::U256> for U256 {
    fn from(x: ethereum_types::U256) -> Self {
        Self(x.to_string())
    }
}

impl TryFrom<U256> for ethereum_types::U256 {
    type Error = &'static str;

    fn try_from(value: U256) -> Result<Self, Self::Error> {
        // TODO Update this message, but check the above ^ first
        value.0.parse().map_err(|_| "bad conversion")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hex(String);

impl From<Vec<u8>> for Hex {
    fn from(data: Vec<u8>) -> Self {
        let hex = hex::encode(data);
        Self(format!("0x{hex}"))
    }
}

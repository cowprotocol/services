use {
    bytes_hex::BytesHex,
    primitive_types::H160,
    serde::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    std::{
        fmt,
        fmt::{Debug, Formatter},
    },
};

/// Order hooks are user-specified Ethereum calls that get executed as part of
/// a pre- or post- interaction.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Hooks {
    #[serde(default)]
    pub pre: Vec<Hook>,
    #[serde(default)]
    pub post: Vec<Hook>,
}

impl Hooks {
    pub fn gas_limit(&self) -> u64 {
        std::iter::empty()
            .chain(&self.pre)
            .chain(&self.post)
            .fold(0_u64, |total, hook| total.saturating_add(hook.gas_limit))
    }
}

/// A user-specified hook.
#[serde_as]
#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hook {
    pub target: H160,
    #[serde_as(as = "BytesHex")]
    pub call_data: Vec<u8>,
    #[serde_as(as = "DisplayFromStr")]
    pub gas_limit: u64,
}

impl Debug for Hook {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Hook")
            .field("target", &self.target)
            .field(
                "call_data",
                &format_args!("0x{}", hex::encode(&self.call_data)),
            )
            .field("gas_limit", &self.gas_limit)
            .finish()
    }
}

pub mod model {
    use {
        ethcontract::H160,
        number::serialization::HexOrDecimalU256,
        primitive_types::U256,
        serde::{Deserialize, Serialize},
        serde_with::serde_as,
    };

    #[serde_as]
    #[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
    pub struct TokenAmount {
        #[serde_as(as = "HexOrDecimalU256")]
        pub amount: U256,
        pub token: H160,
    }

    impl TokenAmount {
        pub fn new<T: Into<U256>>(token: H160, amount: T) -> Self {
            Self {
                amount: amount.into(),
                token,
            }
        }
    }

    /// Whether or not internalizable interactions should be encoded as calldata
    #[derive(Debug, Copy, Clone, Serialize)]
    pub enum InternalizationStrategy {
        #[serde(rename = "Disabled")]
        EncodeAllInteractions,
        #[serde(rename = "Enabled")]
        SkipInternalizableInteraction,
        Unknown,
    }
}

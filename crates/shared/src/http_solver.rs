pub mod model {
    use {ethcontract::H160, primitive_types::U256, serde::Serialize};

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub struct TokenAmount {
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

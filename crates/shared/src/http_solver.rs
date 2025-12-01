pub mod model {
    use {
        alloy::primitives::{Address, U256},
        serde::Serialize,
    };

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub struct TokenAmount {
        pub amount: U256,
        pub token: Address,
    }

    impl TokenAmount {
        pub fn new<T: Into<U256>>(token: Address, amount: T) -> Self {
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

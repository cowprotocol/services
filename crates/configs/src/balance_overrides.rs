use {
    alloy::primitives::{Address, B256, U256},
    serde::Deserialize,
    std::{
        collections::HashMap,
        fmt::{self, Formatter},
    },
};

/// Balance override strategy for a token.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(tag = "type")]
pub enum Strategy {
    /// Balance override strategy for tokens whose balances are stored in a
    /// direct Solidity mapping from token holder to balance amount in the
    /// form `mapping(address holder => uint256 amount)`.
    ///
    /// The strategy is configured with the storage slot [^1] of the mapping.
    ///
    /// [^1]: <https://docs.soliditylang.org/en/latest/internals/layout_in_storage.html#mappings-and-dynamic-arrays>
    SolidityMapping {
        target_contract: Address,
        map_slot: U256,
    },
    /// Strategy computing storage slot for balances based on the Solady library
    /// [^1].
    ///
    /// [^1]: <https://github.com/Vectorized/solady/blob/6122858a3aed96ee9493b99f70a245237681a95f/src/tokens/ERC20.sol#L75-L81>
    SoladyMapping { target_contract: Address },
    /// Strategy that directly uses the storage slot discovered via
    /// debug_traceCall. This is similar to Foundry's `deal` approach where
    /// we trace a balanceOf call to find which storage slot is accessed for
    /// a given account.
    DirectSlot {
        target_contract: Address,
        slot: B256,
    },
    /// Balance override strategy for Aave v3 aTokens whose `balanceOf` applies
    /// a scaling factor (the reserve's normalized income) and whose storage
    /// packs `UserState { uint128 balance; uint128 additionalData }` into a
    /// single slot. The override is resolved at call time by fetching the
    /// current normalized income from the Aave pool and dividing the target
    /// amount by it (ray division), so that `balanceOf` returns the desired
    /// amount. The high 128 bits of the slot (`additionalData`) are zeroed,
    /// which is safe for fresh holders like the spardose.
    ///
    /// Source: <https://github.com/aave-dao/aave-v3-origin/blob/main/src/contracts/protocol/tokenization/base/IncentivizedERC20.sol>
    AaveV3AToken {
        target_contract: Address,
        pool: Address,
        underlying: Address,
    },
}

/// Token configurations for the `BalanceOverriding` component.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct TokenConfiguration(HashMap<Address, Strategy>);

impl TokenConfiguration {
    pub fn inner(&self) -> &HashMap<Address, Strategy> {
        &self.0
    }
}

impl std::fmt::Display for TokenConfiguration {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let format_entry =
            |f: &mut Formatter, (addr, strategy): (&Address, &Strategy)| match strategy {
                Strategy::SolidityMapping {
                    target_contract,
                    map_slot,
                } => write!(
                    f,
                    "SolidityMapping({addr:?}: {target_contract:?}@{map_slot})"
                ),
                Strategy::SoladyMapping { target_contract } => {
                    write!(f, "SoladyMapping({addr:?}: {target_contract})")
                }
                Strategy::DirectSlot {
                    target_contract,
                    slot,
                } => write!(f, "DirectSlot({addr:?}: {target_contract:?}@{slot})"),
                Strategy::AaveV3AToken {
                    target_contract,
                    pool,
                    underlying,
                } => write!(
                    f,
                    "AaveV3AToken({addr:?}: {target_contract:?}, pool={pool:?}, \
                     underlying={underlying:?})"
                ),
            };

        let mut entries = self.0.iter();

        let Some(first) = entries.next() else {
            return Ok(());
        };
        format_entry(f, first)?;

        for entry in entries {
            f.write_str(",")?;
            format_entry(f, entry)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_solidity_mapping() {
        let input = r#"
[0x0000000000000000000000000000000000000001]
type = "SolidityMapping"
target_contract = "0x0000000000000000000000000000000000000002"
map_slot = "0x3"
"#;
        let config: TokenConfiguration = toml::from_str(input).unwrap();
        let strategy = config
            .0
            .get(&Address::from_word(U256::from(1).into()))
            .unwrap();
        assert_eq!(
            *strategy,
            Strategy::SolidityMapping {
                target_contract: Address::from_word(U256::from(2).into()),
                map_slot: U256::from(3),
            }
        );
    }

    #[test]
    fn deserialize_solady_mapping() {
        let input = r#"
[0x0000000000000000000000000000000000000001]
type = "SoladyMapping"
target_contract = "0x0000000000000000000000000000000000000002"
"#;
        let config: TokenConfiguration = toml::from_str(input).unwrap();
        let strategy = config
            .0
            .get(&Address::from_word(U256::from(1).into()))
            .unwrap();
        assert_eq!(
            *strategy,
            Strategy::SoladyMapping {
                target_contract: Address::from_word(U256::from(2).into()),
            }
        );
    }

    #[test]
    fn deserialize_direct_slot() {
        let input = r#"
[0x0000000000000000000000000000000000000001]
type = "DirectSlot"
target_contract = "0x0000000000000000000000000000000000000002"
slot = "0x0000000000000000000000000000000000000000000000000000000000000005"
"#;
        let config: TokenConfiguration = toml::from_str(input).unwrap();
        let strategy = config
            .0
            .get(&Address::from_word(U256::from(1).into()))
            .unwrap();
        assert_eq!(
            *strategy,
            Strategy::DirectSlot {
                target_contract: Address::from_word(U256::from(2).into()),
                slot: B256::from(U256::from(5)),
            }
        );
    }

    #[test]
    fn serialize_roundtrip() {
        let mut map = HashMap::default();
        let addr = Address::from_word(U256::from(1).into());
        map.insert(
            addr,
            Strategy::SolidityMapping {
                target_contract: Address::from_word(U256::from(2).into()),
                map_slot: U256::from(3),
            },
        );
        let config = TokenConfiguration(map);
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: TokenConfiguration = toml::from_str(&serialized).unwrap();
        assert_eq!(config.0, deserialized.0);
    }

    #[test]
    fn deserialize_multiple_tokens() {
        let input = r#"
[0x0000000000000000000000000000000000000001]
type = "SolidityMapping"
target_contract = "0x0000000000000000000000000000000000000002"
map_slot = "0x3"

[0x0000000000000000000000000000000000000004]
type = "SoladyMapping"
target_contract = "0x0000000000000000000000000000000000000005"
"#;
        let config: TokenConfiguration = toml::from_str(input).unwrap();
        assert_eq!(config.0.len(), 2);
    }

    #[test]
    fn deserialize_empty_config() {
        let config: TokenConfiguration = toml::from_str("").unwrap();
        assert!(config.0.is_empty());
    }

    #[test]
    fn deserialize_aave_v3_a_token() {
        use alloy::primitives::address;

        let input = r#"
[0x4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8]
type = "AaveV3AToken"
target_contract = "0x4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8"
pool = "0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2"
underlying = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
"#;
        let config: TokenConfiguration = toml::from_str(input).unwrap();
        let a_token = address!("4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8");
        let strategy = config.0.get(&a_token).unwrap();
        assert_eq!(
            *strategy,
            Strategy::AaveV3AToken {
                target_contract: a_token,
                pool: address!("87870bca3f3fd6335c3f4ce8392d69350b4fa4e2"),
                underlying: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            }
        );
    }

    #[test]
    fn deserialize_unknown_type_fails() {
        let input = r#"
[0x0000000000000000000000000000000000000001]
type = "UnknownStrategy"
target_contract = "0x0000000000000000000000000000000000000002"
"#;
        assert!(toml::from_str::<TokenConfiguration>(input).is_err());
    }
}

use {
    ethcontract::{Address, H256, U256},
    ethrpc::extensions::StateOverride,
    maplit::hashmap,
    std::collections::HashMap,
    web3::signing,
};

/// A component that can provide balance overrides for tokens.
///
/// This allows a wider range of verified quotes to work, even when balances
/// are not available for the quoter.
pub trait BalanceOverriding {
    fn state_override(&self, request: &BalanceOverrideRequest) -> Option<StateOverride>;
}

/// Parameters for computing a balance override request.
pub struct BalanceOverrideRequest {
    /// The token for the override.
    pub token: Address,
    /// The account to override the balance for.
    pub holder: Address,
    /// The token amount (in atoms) to set the balance to.
    pub amount: U256,
}

/// A simple configuration-based balance override provider.
#[derive(Clone, Debug, Default)]
pub struct ConfigurationBalanceOverrides(HashMap<Address, Strategy>);

#[derive(Clone, Debug)]
pub enum Strategy {
    Mapping { slot: U256 },
}

impl ConfigurationBalanceOverrides {
    pub fn new(config: HashMap<Address, Strategy>) -> Self {
        Self(config)
    }
}

impl BalanceOverriding for ConfigurationBalanceOverrides {
    fn state_override(&self, request: &BalanceOverrideRequest) -> Option<StateOverride> {
        let strategy = self.0.get(&request.token)?;
        match strategy {
            Strategy::Mapping { slot } => Some(StateOverride {
                state_diff: Some(hashmap! {
                    address_mapping_storage_slot(slot, &request.holder) => request.amount,
                }),
                ..Default::default()
            }),
        }
    }
}

/// Computes the storage slot where the value is stored for Solidity mappings
/// of the form `mapping(address => ...)`.
///
/// See <https://docs.soliditylang.org/en/latest/internals/layout_in_storage.html#mappings-and-dynamic-arrays>.
fn address_mapping_storage_slot(slot: &U256, address: &Address) -> H256 {
    let mut buf = [0; 64];
    buf[12..32].copy_from_slice(address.as_fixed_bytes());
    slot.to_big_endian(&mut buf[32..64]);
    H256(signing::keccak256(&buf))
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[test]
    fn balance_override_computation() {
        let balance_overrides = ConfigurationBalanceOverrides::new(hashmap! {
            addr!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB") => Strategy::Mapping {
                slot: U256::from(0),
            },
        });

        assert_eq!(
            balance_overrides.state_override(&BalanceOverrideRequest {
                token: addr!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                holder: addr!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
                amount: 0x42_u64.into(),
            }),
            Some(StateOverride {
                state_diff: Some(hashmap! {
                    H256(hex!("fca351f4d96129454cfc8ef7930b638ac71fea35eb69ee3b8d959496beb04a33")) => 0x42_u64.into()
                }),
                ..Default::default()
            }),
        );

        // You can verify the state override computation is correct by running:
        // ```
        // curl -X POST $RPC -H 'Content-Type: application/data' --data '{
        //   "jsonrpc": "2.0",
        //   "id": 0,
        //   "method": "eth_call",
        //   "params": [
        //     {
        //       "to": "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
        //       "data": "0x70a08231000000000000000000000000d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
        //     },
        //     "latest",
        //     {
        //       "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
        //         "stateDiff": {
        //           "0xfca351f4d96129454cfc8ef7930b638ac71fea35eb69ee3b8d959496beb04a33":
        //             "0x0000000000000000000000000000000000000000000000000000000000000042"
        //         }
        //       }
        //     }
        //   ]
        // }'
        // ```
    }

    #[test]
    fn balance_overrides_none_for_unknown_tokens() {
        let balance_overrides = ConfigurationBalanceOverrides::default();
        assert_eq!(
            balance_overrides.state_override(&BalanceOverrideRequest {
                token: addr!("0000000000000000000000000000000000000000"),
                holder: addr!("0000000000000000000000000000000000000001"),
                amount: U256::zero(),
            }),
            None,
        );
    }
}

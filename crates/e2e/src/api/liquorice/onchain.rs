pub use order::signature::DomainSeparator;
pub mod order {
    pub use {signature::Signature, single::Single};
    pub mod single {
        use {
            super::signature::{DomainSeparator, Signature},
            crate::setup::TestAccount,
            alloy::{
                primitives::{Address, B256, U256, b256, keccak256},
                signers::{SignerSync, local::PrivateKeySigner},
                sol,
                sol_types::{SolType, SolValue},
            },
        };

        pub type OrderDataPart1Sol = sol! {
            tuple(
                bytes32, // SINGLE_ORDER_TYPED_HASH
                bytes32, // keccak256(rfq_id)
                uint256, // nonce
                address, // trader
            )
        };

        pub type OrderDataPart2Sol = sol! {
            tuple(
                address, // effectiveTrader
                address, // baseToken
                address, // quoteToken
                uint256, // baseTokenAmount
                uint256, // quoteTokenAmount
                uint256, // minFillAmount
                uint256, // quoteExpiry
                address, // recipient
            )
        };

        pub type OrderDataEIP712Sol = sol! {
            tuple(
                bytes2, // '\x19\x01'
                bytes32, // DOMAIN_SEPARATOR()
                bytes32, // order data hash
            )
        };

        pub struct Single {
            pub rfq_id: String,
            pub nonce: U256,
            pub trader: Address,
            pub effective_trader: Address,
            pub base_token: Address,
            pub quote_token: Address,
            pub base_token_amount: U256,
            pub quote_token_amount: U256,
            pub min_fill_amount: U256,
            pub quote_expiry: U256,
            pub recipient: Address,
        }

        impl Single {
            // See <https://etherscan.io/address/0x0448633eb8b0a42efed924c42069e0dcf08fb552#code#F8#L41>
            const LIQUORICE_SINGLE_ORDER_TYPEHASH: B256 =
                b256!("d28e809b708f5ee38be8347d6d869d8232493c094ab2dde98369e4102369a99d");

            pub fn sign(
                &self,
                domain_separator: &DomainSeparator,
                hash: B256,
                signer: &TestAccount,
            ) -> Signature {
                let order_data_eip712 = OrderDataEIP712Sol::abi_encode_packed(&(
                    [0x19, 0x01],
                    domain_separator.0,
                    hash,
                ));

                let hashed_signing_message = keccak256(order_data_eip712);

                let signer = PrivateKeySigner::from_slice(signer.private_key()).unwrap();
                let signature = signer.sign_hash_sync(&hashed_signing_message).unwrap();

                Signature {
                    signature_type: 3,   // EIP-712
                    transfer_command: 1, // Transfer command standard
                    signature,
                }
            }

            // See <https://etherscan.io/address/0x0448633eb8b0a42efed924c42069e0dcf08fb552#code#F13#L180>
            pub fn hash(&self) -> B256 {
                let order_part_1 = OrderDataPart1Sol::abi_encode_sequence(&(
                    Self::LIQUORICE_SINGLE_ORDER_TYPEHASH,
                    keccak256(self.rfq_id.to_string().abi_encode()),
                    self.nonce,
                    self.trader,
                ));

                let order_part_2 = OrderDataPart2Sol::abi_encode_sequence(&(
                    self.effective_trader,
                    self.base_token,
                    self.quote_token,
                    self.base_token_amount,
                    self.quote_token_amount,
                    self.min_fill_amount,
                    self.quote_expiry,
                    self.recipient,
                ));

                keccak256([&order_part_1[..], &order_part_2[..]].concat())
            }
        }

        #[cfg(test)]
        mod tests {
            use {
                super::Single,
                alloy::primitives::{U256, address},
            };

            #[test]
            fn test_order_hash() {
                let order = Single {
                    rfq_id: "c99d2e3f-702b-49c9-8bb8-43775770f2f3".to_string(),
                    nonce: U256::from(0),
                    trader: address!("48426Ef27C3555D44DACDD647D8f9bd0A7C06155"),
                    effective_trader: address!("033F42e758cEbEbC70Ee147F56ff92C9f7CA45F4"),
                    base_token: address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1"),
                    quote_token: address!("2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f"),
                    base_token_amount: U256::from(1),
                    quote_token_amount: U256::from(2),
                    min_fill_amount: U256::from(1),
                    quote_expiry: U256::from(1715787259),
                    recipient: address!("033F42e758cEbEbC70Ee147F56ff92C9f7CA45F4"),
                };

                let hash = order.hash();

                assert_eq!(
                    "d11023397b6e58bf8137e479bc552f06eb3b7527652528a047eae91bb391858d",
                    const_hex::encode(hash)
                );
            }
        }
    }

    pub mod signature {
        use {
            alloy::{
                primitives::{Address, B256, U256, keccak256},
                sol,
                sol_types::SolType,
            },
            std::sync::LazyLock,
        };

        pub type DomainSeparatorSol = sol! {
            tuple(
                bytes32, // EIP712_DOMAIN_TYPEHASH
                bytes32, // keccak(domain.name)
                bytes32, // keccak(domain.version)
                uint256, // block.chainId
                address, // address(this)
            )
        };

        #[derive(Copy, Clone, Default, Eq, PartialEq)]
        pub struct DomainSeparator(pub B256);

        impl DomainSeparator {
            pub fn new(chain_id: u64, contract_address: Address) -> Self {
                static DOMAIN_TYPE_HASH: LazyLock<B256> = LazyLock::new(|| {
                    keccak256(
                        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
                    )
                });

                static DOMAIN_NAME: LazyLock<B256> =
                    LazyLock::new(|| keccak256(b"LiquoriceSettlement"));

                static DOMAIN_VERSION: LazyLock<B256> = LazyLock::new(|| keccak256(b"1"));

                Self(keccak256(DomainSeparatorSol::abi_encode_sequence(&(
                    (*DOMAIN_TYPE_HASH),
                    (*DOMAIN_NAME),
                    (*DOMAIN_VERSION),
                    U256::from(chain_id),
                    contract_address,
                ))))
            }
        }
        pub struct Signature {
            pub signature_type: u8,
            pub transfer_command: u8,
            pub signature: alloy::primitives::Signature,
        }
    }
}

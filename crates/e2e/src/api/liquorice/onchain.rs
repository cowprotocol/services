pub use order::signature::DomainSeparator;
pub mod order {
    pub use {signature::Signature, single::Single};
    pub mod single {
        use {
            super::signature::{DomainSeparator, Signature},
            crate::setup::TestAccount,
            ethcontract::common::abi::{Token, encode},
            hex_literal::hex,
            secp256k1::SecretKey,
            web3::{
                signing,
                signing::{Key, SecretKeyRef},
                types::{H160, U256},
            },
        };

        pub struct Single {
            pub rfq_id: String,
            pub nonce: U256,
            pub trader: H160,
            pub effective_trader: H160,
            pub base_token: H160,
            pub quote_token: H160,
            pub base_token_amount: U256,
            pub quote_token_amount: U256,
            pub min_fill_amount: U256,
            pub quote_expiry: U256,
            pub recipient: H160,
        }

        impl Single {
            // See <https://etherscan.io/address/0x0448633eb8b0a42efed924c42069e0dcf08fb552#code#F8#L41>
            const LIQUORICE_SINGLE_ORDER_TYPEHASH: [u8; 32] =
                hex!("d28e809b708f5ee38be8347d6d869d8232493c094ab2dde98369e4102369a99d");

            pub fn sign(
                &self,
                domain_separator: &DomainSeparator,
                hash: [u8; 32],
                signer: &TestAccount,
            ) -> Signature {
                let hashed_signing_message = {
                    let mut msg = [0u8; 66];
                    msg[0..2].copy_from_slice(&[0x19, 0x01]);
                    msg[2..34].copy_from_slice(&domain_separator.0);
                    msg[34..66].copy_from_slice(hash.as_ref());
                    signing::keccak256(&msg)
                };

                let signature =
                    SecretKeyRef::from(&SecretKey::from_slice(signer.private_key()).unwrap())
                        .sign(&hashed_signing_message, None)
                        .unwrap();

                Signature {
                    signature_type: 3,   // EIP-712
                    transfer_command: 1, // Transfer command standard
                    signature_bytes: {
                        let mut sig_bytes = Vec::new();
                        sig_bytes.extend_from_slice(&signature.r.0);
                        sig_bytes.extend_from_slice(&signature.s.0);
                        sig_bytes.extend_from_slice(&(signature.v as u8).to_be_bytes());
                        ethcontract::Bytes(sig_bytes)
                    },
                }
            }

            // See <https://etherscan.io/address/0x0448633eb8b0a42efed924c42069e0dcf08fb552#code#F13#L180>
            pub fn hash(&self) -> [u8; 32] {
                let order_data_part_1 = {
                    let mut hash_data = [0u8; 128];

                    hash_data[0..32].copy_from_slice(&Self::LIQUORICE_SINGLE_ORDER_TYPEHASH);
                    hash_data[32..64].copy_from_slice(
                        signing::keccak256(
                            encode(&[Token::String(self.rfq_id.clone())]).as_slice(),
                        )
                        .as_slice(),
                    );
                    self.nonce.to_big_endian(&mut hash_data[64..96]);
                    hash_data[108..128].clone_from_slice(self.trader.as_fixed_bytes());
                    hash_data
                };

                let order_data_part_2 = {
                    let mut hash_data = [0u8; 256];

                    hash_data[12..32].copy_from_slice(self.effective_trader.as_fixed_bytes());
                    hash_data[44..64].copy_from_slice(self.base_token.as_fixed_bytes());
                    hash_data[76..96].copy_from_slice(self.quote_token.as_fixed_bytes());
                    self.base_token_amount
                        .to_big_endian(&mut hash_data[96..128]);
                    self.quote_token_amount
                        .to_big_endian(&mut hash_data[128..160]);
                    self.min_fill_amount.to_big_endian(&mut hash_data[160..192]);
                    self.quote_expiry.to_big_endian(&mut hash_data[192..224]);
                    hash_data[236..256].copy_from_slice(self.recipient.as_fixed_bytes());
                    hash_data
                };

                signing::keccak256(
                    [&order_data_part_1[..], &order_data_part_2[..]]
                        .concat()
                        .as_slice(),
                )
            }

            pub fn as_tuple(
                &self,
            ) -> (
                String,
                U256,
                H160,
                H160,
                H160,
                H160,
                U256,
                U256,
                U256,
                U256,
                H160,
            ) {
                (
                    self.rfq_id.clone(),
                    self.nonce,
                    self.trader,
                    self.effective_trader,
                    self.base_token,
                    self.quote_token,
                    self.base_token_amount,
                    self.quote_token_amount,
                    self.min_fill_amount,
                    self.quote_expiry,
                    self.recipient,
                )
            }
        }

        #[cfg(test)]
        mod tests {
            use {
                super::Single,
                hex_literal::hex,
                web3::types::{H160, U256},
            };

            #[test]
            fn test_order_hash() {
                let order = Single {
                    rfq_id: "c99d2e3f-702b-49c9-8bb8-43775770f2f3".to_string(),
                    nonce: U256::from(0),
                    trader: H160::from(hex!("48426Ef27C3555D44DACDD647D8f9bd0A7C06155")),
                    effective_trader: H160::from(hex!("033F42e758cEbEbC70Ee147F56ff92C9f7CA45F4")),
                    base_token: H160::from(hex!("82aF49447D8a07e3bd95BD0d56f35241523fBab1")),
                    quote_token: H160::from(hex!("2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f")),
                    base_token_amount: U256::from(1),
                    quote_token_amount: U256::from(2),
                    min_fill_amount: U256::from(1),
                    quote_expiry: U256::from(1715787259),
                    recipient: H160::from(hex!("033F42e758cEbEbC70Ee147F56ff92C9f7CA45F4")),
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
            autopilot::domain::eth::H160,
            ethcontract::common::abi::{Token, encode},
            std::sync::LazyLock,
            web3::signing,
        };

        #[derive(Copy, Clone, Default, Eq, PartialEq)]
        pub struct DomainSeparator(pub [u8; 32]);

        impl DomainSeparator {
            pub fn new(chain_id: u64, contract_address: H160) -> Self {
                static DOMAIN_TYPE_HASH: LazyLock<[u8; 32]> = LazyLock::new(|| {
                    signing::keccak256(
                        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
                    )
                });

                static DOMAIN_NAME: LazyLock<[u8; 32]> =
                    LazyLock::new(|| signing::keccak256(b"LiquoriceSettlement"));

                static DOMAIN_VERSION: LazyLock<[u8; 32]> =
                    LazyLock::new(|| signing::keccak256(b"1"));

                let abi_encode_string = encode(&[
                    Token::Uint((*DOMAIN_TYPE_HASH).into()),
                    Token::Uint((*DOMAIN_NAME).into()),
                    Token::Uint((*DOMAIN_VERSION).into()),
                    Token::Uint(chain_id.into()),
                    Token::Address(contract_address),
                ]);

                Self(signing::keccak256(abi_encode_string.as_slice()))
            }
        }
        #[derive(Default)]
        pub struct Signature {
            pub signature_type: u8,
            pub transfer_command: u8,
            pub signature_bytes: ethcontract::Bytes<Vec<u8>>,
        }

        impl Signature {
            pub fn as_tuple(&self) -> (u8, u8, ethcontract::Bytes<Vec<u8>>) {
                (
                    self.signature_type,
                    self.transfer_command,
                    self.signature_bytes.clone(),
                )
            }
        }
    }
}

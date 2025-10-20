pub use order::signature::DomainSeparator;
pub mod order {
    pub use {signature::Signature, single::Single};
    pub mod single {
        use {
            super::signature::{DomainSeparator, Signature},
            crate::setup::TestAccount,
            alloy::{
                primitives::{Address, B256, U256, b256, keccak256},
                sol,
                sol_types::{SolType, SolValue},
            },
            secp256k1::SecretKey,
            web3::{
                signing,
                signing::{Key, SecretKeyRef},
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

pub mod reference {
    //     use std::time::Duration;
    //
    //     use alloy::{
    //         primitives::{keccak256, Address, PrimitiveSignature, B256, U256},
    //         sol,
    //         sol_types::{SolType, SolValue},
    //     };
    //     use chrono::{serde::ts_seconds, DateTime, Utc};
    //     use serde::{Deserialize, Serialize};
    //     use serde_with::{hex::Hex, serde_as, DisplayFromStr};
    //     use uuid::Uuid;
    //
    //     use crate::{
    //         domain::eth::ChainId,
    //         protocol::messages::maker::{
    //             rfq::RFQMessage,
    //             rfq_quote::eip712::{DomainSeparator, OrderDataEIP712Sol,
    // SINGLE_ORDER_TYPED_HASH},         },
    //         utils::serialization::{signature::SignatureHex,
    // u256::DecimalU256},     };
    //
    //     #[serde_as]
    //     #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize,
    // Deserialize)]     #[serde(rename_all = "camelCase")]
    //     pub struct QuoteLevelLite {
    //         /// Date and time when the quote will expire
    //         #[serde(with = "ts_seconds")]
    //         pub expiry: DateTime<Utc>,
    //         /// Address of the Liquorice Settlement Contract
    //         #[serde_as(as = "DisplayFromStr")]
    //         pub settlement_contract: Address,
    //         /// Address of baseToken recipient.
    //         /// If absent, signer assumed to be the recipient
    //         #[serde_as(as = "Option<DisplayFromStr>")]
    //         pub recipient: Option<Address>,
    //         /// Address of the RFQ signer
    //         #[serde_as(as = "DisplayFromStr")]
    //         pub signer: Address,
    //         /// Address of the EIP-1271 verifying contract
    //         #[serde_as(as = "Option<DisplayFromStr>")]
    //         pub eip1271_verifier: Option<Address>,
    //         /// Base token address
    //         #[serde_as(as = "DisplayFromStr")]
    //         pub base_token: Address,
    //         /// Quote token address
    //         #[serde_as(as = "DisplayFromStr")]
    //         pub quote_token: Address,
    //         /// Base Token amount
    //         #[serde_as(as = "DecimalU256")]
    //         pub base_token_amount: U256,
    //         /// Quote Token amount
    //         #[serde_as(as = "DecimalU256")]
    //         pub quote_token_amount: U256,
    //         /// Minimal amount for partial fill.
    //         /// If omitted, order can not be filled partially.
    //         #[serde(skip_serializing_if = "Option::is_none")]
    //         #[serde_as(as = "Option<DecimalU256>")]
    //         pub min_quote_token_amount: Option<U256>,
    //         /// Quote signature
    //         #[serde_as(as = "Option<SignatureHex>")]
    //         pub signature: Option<PrimitiveSignature>,
    //     }
    //
    //     impl QuoteLevelLite {
    //         pub fn expiration_delta(&self) -> Result<Duration, String> {
    //             (self.expiry - Utc::now())
    //                 .to_std()
    //                 .map_err(|e| e.to_string())
    //         }
    //
    //         pub fn hash(&self, rfq: &RFQMessage) -> B256 {
    //             SignaturePayload::builder()
    //                 .quote_level(self)
    //                 .rfq_message(rfq)
    //                 .build()
    //                 .hash()
    //         }
    //     }
    //
    //     #[serde_as]
    //     #[derive(Debug, Serialize)]
    //     pub struct SignaturePayload {
    //         chain_id: ChainId,
    //         rfq_id: Uuid,
    //         settlement_contract: Address,
    //         #[serde_as(as = "Hex")]
    //         nonce: [u8; 32],
    //         trader: Address,
    //         effective_trader: Address,
    //         base_token: Address,
    //         quote_token: Address,
    //         #[serde_as(as = "DisplayFromStr")]
    //         base_token_amount: U256,
    //         #[serde_as(as = "DisplayFromStr")]
    //         quote_token_amount: U256,
    //         #[serde_as(as = "DisplayFromStr")]
    //         min_quote_token_amount: U256,
    //         #[serde(with = "ts_seconds")]
    //         expiry: DateTime<Utc>,
    //         recipient: Address,
    //     }
    //
    //     pub type OrderDataPart1Sol = sol! {
    //     tuple(
    //         bytes32, // SINGLE_ORDER_TYPED_HASH
    //         bytes32, // keccak256(rfq_id)
    //         uint256, // nonce
    //         address, // trader
    //     )
    // };
    //
    //     pub type OrderDataPart2Sol = sol! {
    //     tuple(
    //         address, // effectiveTrader
    //         address, // baseToken
    //         address, // quoteToken
    //         uint256, // baseTokenAmount
    //         uint256, // quoteTokenAmount
    //         uint256, // minFillAmount
    //         uint256, // quoteExpiry
    //         address, // recipient
    //     )
    // };
    //
    //     #[derive(Default)]
    //     pub struct SignaturePayloadBuilder<'a> {
    //         rfq_message: Option<&'a RFQMessage>,
    //         quote_level: Option<&'a QuoteLevelLite>,
    //     }
    //
    //     impl SignaturePayload {
    //         pub fn builder<'a>() -> SignaturePayloadBuilder<'a> {
    //             SignaturePayloadBuilder::default()
    //         }
    //
    //         pub fn hash(&self) -> B256 {
    //             let domain_separator_hash =
    // DomainSeparator::hash(self.chain_id, self.settlement_contract);
    //
    //             let order_part_1 = OrderDataPart1Sol::abi_encode_sequence(&(
    //                 *SINGLE_ORDER_TYPED_HASH,
    //                 keccak256(self.rfq_id.to_string().abi_encode()),
    //                 U256::from_be_slice(self.nonce.as_slice()),
    //                 self.trader,
    //             ));
    //
    //             let order_part_2 = OrderDataPart2Sol::abi_encode_sequence(&(
    //                 self.effective_trader,
    //                 self.base_token,
    //                 self.quote_token,
    //                 self.base_token_amount,
    //                 self.quote_token_amount,
    //                 self.min_quote_token_amount,
    //                 U256::from(self.expiry.timestamp()),
    //                 self.recipient,
    //             ));
    //
    //             let order_data_hash = keccak256([&order_part_1[..],
    // &order_part_2[..]].concat());
    //
    //             let order_data_eip712 =
    // OrderDataEIP712Sol::abi_encode_packed(&(                 [0x19,
    // 0x01],                 domain_separator_hash,
    //                 order_data_hash,
    //             ));
    //
    //             keccak256(order_data_eip712)
    //         }
    //     }
    //
    //     impl<'a> SignaturePayloadBuilder<'a> {
    //         pub fn rfq_message(mut self, rfq_message: &'a RFQMessage) -> Self
    // {             self.rfq_message = Some(rfq_message);
    //             self
    //         }
    //
    //         pub fn quote_level(mut self, payload: &'a QuoteLevelLite) -> Self
    // {             self.quote_level = Some(payload);
    //             self
    //         }
    //
    //         pub fn build(&self) -> SignaturePayload {
    //             let rfq_message = self.rfq_message.unwrap();
    //             let quote_level = self.quote_level.unwrap();
    //
    //             SignaturePayload {
    //                 chain_id: rfq_message.chain_id,
    //                 nonce: rfq_message.nonce,
    //                 trader: rfq_message.trader,
    //                 effective_trader: rfq_message.effective_trader,
    //                 rfq_id: rfq_message.rfq_id,
    //                 settlement_contract: quote_level.settlement_contract,
    //                 base_token: quote_level.base_token,
    //                 quote_token: quote_level.quote_token,
    //                 base_token_amount: quote_level.base_token_amount,
    //                 quote_token_amount: quote_level.quote_token_amount,
    //                 min_quote_token_amount: quote_level
    //                     .min_quote_token_amount
    //                     .unwrap_or(quote_level.quote_token_amount),
    //                 expiry: quote_level.expiry,
    //                 recipient:
    // quote_level.recipient.unwrap_or(quote_level.signer),             }
    //         }
    //     }
    //
    //     #[cfg(test)]
    //     mod tests {
    //         use crate::{
    //
    // protocol::messages::maker::rfq_quote::quote_level_lite::SignaturePayload,
    //             tests::fixtures::maker_api::{rfq_message, rfq_quote_message},
    //         };
    //
    //         #[test]
    //         fn test_rfq_quote_level_signature_hash() {
    //             let rfq = rfq_message::fixture();
    //             let rfq_quote = rfq_quote_message::fixture();
    //
    //             let signature_payload = SignaturePayload::builder()
    //                 .quote_level(rfq_quote.levels[0].as_lite())
    //                 .rfq_message(&rfq)
    //                 .build();
    //
    //             let hash = signature_payload.hash();
    //
    //             // Generated hash should match the expected hash
    //             assert_eq!(
    //                 format!("{hash}"),
    //
    // "0x2342c2e81befd9dda11c9e769d6d867e347d5b84a0137bf9fa31acbe7ee4f5ac"
    //             );
    //         }
    //     }
    //
}

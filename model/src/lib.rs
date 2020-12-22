//! Contains models that are shared between the orderbook and the solver.

pub mod h160_hexadecimal;
pub mod order;
pub mod u256_decimal;

use ethabi::{encode, Token};
use hex::{FromHex, FromHexError};
use lazy_static::lazy_static;
use primitive_types::{H160, H256};
use serde::{de, Deserialize, Serialize};
use std::fmt;
use web3::signing;

#[derive(Eq, PartialEq, Clone, Copy, Debug, Default)]
pub struct Signature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut bytes = [0u8; 2 + 65 * 2];
        bytes[..2].copy_from_slice(b"0x");
        // Can only fail if the buffer size does not match but we know it is correct.
        hex::encode_to_slice(self.r, &mut bytes[2..66]).unwrap();
        hex::encode_to_slice(self.s, &mut bytes[66..130]).unwrap();
        hex::encode_to_slice([self.v], &mut bytes[130..132]).unwrap();
        // Hex encoding is always valid utf8.
        let str = std::str::from_utf8(&bytes).unwrap();
        serializer.serialize_str(str)
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor {}
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Signature;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "the 65 signature bytes as a hex encoded string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let s = s.strip_prefix("0x").ok_or_else(|| {
                    de::Error::custom(format!(
                        "{:?} can't be decoded as hex signature because it does not start with '0x'",
                        s
                    ))
                })?;
                let mut bytes = [0u8; 65];
                hex::decode_to_slice(s, &mut bytes).map_err(|err| {
                    de::Error::custom(format!(
                        "failed to decode {:?} as hex signature: {}",
                        s, err
                    ))
                })?;
                Ok(Signature {
                    r: H256::from_slice(&bytes[..32]),
                    s: H256::from_slice(&bytes[32..64]),
                    v: bytes[64],
                })
            }
        }

        deserializer.deserialize_str(Visitor {})
    }
}

/// Erc20 token pair specified by two contract addresses.
#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash, Ord, PartialOrd)]
pub struct TokenPair(H160, H160);

impl TokenPair {
    /// Create a new token pair from two addresses.
    /// The addresses must not be the equal.
    pub fn new(token_a: H160, token_b: H160) -> Option<Self> {
        match token_a.cmp(&token_b) {
            std::cmp::Ordering::Less => Some(Self(token_a, token_b)),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(Self(token_b, token_a)),
        }
    }

    /// The first address is always the lower one.
    /// The addresses are never equal.
    pub fn get(&self) -> (H160, H160) {
        (self.0, self.1)
    }
}

#[derive(Copy, Eq, PartialEq, Clone, Default)]
pub struct DomainSeparator(pub [u8; 32]);

impl std::str::FromStr for DomainSeparator {
    type Err = FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(FromHex::from_hex(s)?))
    }
}

impl std::fmt::Debug for DomainSeparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut hex = [0u8; 64];
        // Unwrap because we know the length is correct.
        hex::encode_to_slice(self.0, &mut hex).unwrap();
        // Unwrap because we know it is valid utf8.
        f.write_str(std::str::from_utf8(&hex).unwrap())
    }
}

impl DomainSeparator {
    pub fn get_domain_separator(chain_id: u64, contract_address: H160) -> Self {
        lazy_static! {
            /// The EIP-712 domain name used for computing the domain separator.
            static ref DOMAIN_NAME: [u8; 32] = signing::keccak256(b"Gnosis Protocol");

            /// The EIP-712 domain version used for computing the domain separator.
            static ref DOMAIN_VERSION: [u8; 32] = signing::keccak256(b"v2");

            /// The EIP-712 domain type used computing the domain separator.
            static ref DOMAIN_TYPE_HASH: [u8; 32] = signing::keccak256(
                b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
            );
        }
        let abi_encode_string = encode(&[
            Token::Uint((*DOMAIN_TYPE_HASH).into()),
            Token::Uint((*DOMAIN_NAME).into()),
            Token::Uint((*DOMAIN_VERSION).into()),
            Token::Uint(chain_id.into()),
            Token::Address(contract_address),
        ]);

        DomainSeparator(signing::keccak256(abi_encode_string.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn domain_separator_rinkeby() {
        let contract_address: H160 = hex!("91D6387ffbB74621625F39200d91a50386C9Ab15").into();
        let chain_id: u64 = 4;
        let domain_separator_rinkeby =
            DomainSeparator::get_domain_separator(chain_id, contract_address);
        // domain separator is taken from rinkeby deployment at address 91D6387ffbB74621625F39200d91a50386C9Ab15
        let expected_domain_separator: DomainSeparator = DomainSeparator(hex!(
            "9d7e07ef92761aa9453ae5ff25083a2b19764131b15295d3c7e89f1f1b8c67d9"
        ));
        assert_eq!(domain_separator_rinkeby, expected_domain_separator);
    }

    #[test]
    fn token_pair_is_sorted() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let pair_0 = TokenPair::new(token_a, token_b).unwrap();
        let pair_1 = TokenPair::new(token_b, token_a).unwrap();
        assert_eq!(pair_0, pair_1);
        assert_eq!(pair_0.get(), pair_1.get());
        assert_eq!(pair_0.get().0, token_a);
    }

    #[test]
    fn token_pair_cannot_be_equal() {
        let token = H160::from_low_u64_be(1);
        assert_eq!(TokenPair::new(token, token), None);
    }
}

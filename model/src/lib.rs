//! Contains models that are shared between the orderbook and the solver.

pub mod appdata_hexadecimal;
pub mod h160_hexadecimal;
pub mod order;
pub mod trade;
pub mod u256_decimal;

use ethabi::{encode, Token};
use hex::{FromHex, FromHexError};
use lazy_static::lazy_static;
use primitive_types::{H160, H256};
use serde::{de, Deserialize, Serialize};
use std::fmt;
use web3::signing::{self};
use web3::signing::{Key, SecretKeyRef};
use web3::types::Recovery;

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Hash)]
pub struct Signature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
}

fn hashed_eip712_message(domain_separator: &DomainSeparator, struct_hash: &[u8; 32]) -> [u8; 32] {
    let mut message = [0u8; 66];
    message[0..2].copy_from_slice(&[0x19, 0x01]);
    message[2..34].copy_from_slice(&domain_separator.0);
    message[34..66].copy_from_slice(struct_hash);
    signing::keccak256(&message)
}

fn hashed_ethsign_message(domain_separator: &DomainSeparator, struct_hash: &[u8; 32]) -> [u8; 32] {
    let mut message = [0u8; 60];
    message[..28].copy_from_slice(b"\x19Ethereum Signed Message:\n32");
    message[28..].copy_from_slice(&hashed_eip712_message(domain_separator, struct_hash));
    signing::keccak256(&message)
}

fn hashed_signing_message(
    signing_scheme: SigningScheme,
    domain_separator: &DomainSeparator,
    struct_hash: &[u8; 32],
) -> [u8; 32] {
    match signing_scheme {
        SigningScheme::Eip712 => hashed_eip712_message(domain_separator, struct_hash),
        SigningScheme::EthSign => hashed_ethsign_message(domain_separator, struct_hash),
    }
}

impl Signature {
    /// r + s + v
    pub fn to_bytes(&self) -> [u8; 65] {
        let mut bytes = [0u8; 65];
        bytes[..32].copy_from_slice(self.r.as_bytes());
        bytes[32..64].copy_from_slice(self.s.as_bytes());
        bytes[64] = self.v;
        bytes
    }

    pub fn from_bytes(bytes: &[u8; 65]) -> Self {
        Signature {
            r: H256::from_slice(&bytes[..32]),
            s: H256::from_slice(&bytes[32..64]),
            v: bytes[64],
        }
    }

    pub fn validate(
        &self,
        signing_scheme: SigningScheme,
        domain_separator: &DomainSeparator,
        struct_hash: &[u8; 32],
    ) -> Option<H160> {
        let message = hashed_signing_message(signing_scheme, domain_separator, struct_hash);
        let recovery = Recovery::new(message, self.v as u64, self.r, self.s);
        let (signature, recovery_id) = recovery.as_signature()?;
        signing::recover(&message, &signature, recovery_id).ok()
    }

    pub fn sign(
        signing_scheme: SigningScheme,
        domain_separator: &DomainSeparator,
        struct_hash: &[u8; 32],
        key: SecretKeyRef,
    ) -> Self {
        let message = hashed_signing_message(signing_scheme, domain_separator, struct_hash);
        // Unwrap because the only error is for invalid messages which we don't create.
        let signature = key.sign(&message, None).unwrap();
        Self {
            v: signature.v as u8,
            r: signature.r,
            s: signature.s,
        }
    }
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut bytes = [0u8; 2 + 65 * 2];
        bytes[..2].copy_from_slice(b"0x");
        // Can only fail if the buffer size does not match but we know it is correct.
        hex::encode_to_slice(&self.to_bytes(), &mut bytes[2..]).unwrap();
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
                Ok(Signature::from_bytes(&bytes))
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

    /// Used to determine if `token` is among the pair.
    pub fn contains(&self, token: &H160) -> bool {
        self.0 == *token || self.1 == *token
    }

    /// The first address is always the lower one.
    /// The addresses are never equal.
    pub fn get(&self) -> (H160, H160) {
        (self.0, self.1)
    }
}

impl Default for TokenPair {
    fn default() -> Self {
        Self::new(H160::from_low_u64_be(0), H160::from_low_u64_be(1)).unwrap()
    }
}

impl IntoIterator for TokenPair {
    type Item = H160;
    type IntoIter = std::iter::Chain<std::iter::Once<H160>, std::iter::Once<H160>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.0).chain(std::iter::once(self.1))
    }
}

impl<'a> IntoIterator for &'a TokenPair {
    type Item = &'a H160;
    type IntoIter = std::iter::Chain<std::iter::Once<&'a H160>, std::iter::Once<&'a H160>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.0).chain(std::iter::once(&self.1))
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
        let expected_domain_separator = DomainSeparator(hex!(
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

    #[test]
    fn token_pair_iterator() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        let mut iter = (&pair).into_iter();
        assert_eq!(iter.next(), Some(&token_a));
        assert_eq!(iter.next(), Some(&token_b));
        assert_eq!(iter.next(), None);

        let mut iter = pair.into_iter();
        assert_eq!(iter.next(), Some(token_a));
        assert_eq!(iter.next(), Some(token_b));
        assert_eq!(iter.next(), None);
    }
}

// https://github.com/rust-lang/rust-clippy/issues/9782
#![allow(clippy::needless_borrow)]

//! Contains models that are shared between the orderbook and the solver.

pub mod app_id;
pub mod auction;
pub mod bytes_hex;
pub mod interaction;
pub mod order;
pub mod quote;
pub mod ratio_as_decimal;
pub mod signature;
pub mod solver_competition;
pub mod time;
pub mod trade;
pub mod u256_decimal;

use hex::{FromHex, FromHexError};
use lazy_static::lazy_static;
use primitive_types::H160;
use std::fmt;
use web3::{
    ethabi::{encode, Token},
    signing,
};

/// Erc20 token pair specified by two contract addresses.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

    /// Returns the token in the pair which is not the one passed in, or None if token passed in is not part of the pair
    pub fn other(&self, token: &H160) -> Option<H160> {
        if &self.0 == token {
            Some(self.1)
        } else if &self.1 == token {
            Some(self.0)
        } else {
            None
        }
    }

    /// The first address is always the lower one.
    /// The addresses are never equal.
    pub fn get(&self) -> (H160, H160) {
        (self.0, self.1)
    }

    /// Lowest element according to Ord trait.
    pub fn first_ord() -> Self {
        Self(
            H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
        )
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

#[derive(Copy, Clone, Default, Eq, PartialEq)]
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
    pub fn new(chain_id: u64, contract_address: H160) -> Self {
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

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolvableOrders {
    pub orders: Vec<order::Order>,
    pub latest_settlement_block: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;
    use std::{cmp::Ordering, str::FromStr};

    #[test]
    fn domain_separator_from_str() {
        assert!(DomainSeparator::from_str(
            "9d7e07ef92761aa9453ae5ff25083a2b19764131b15295d3c7e89f1f1b8c67d9"
        )
        .is_ok());
    }

    #[test]
    fn domain_separator_goerli() {
        let contract_address: H160 = hex!("9008D19f58AAbD9eD0D60971565AA8510560ab41").into(); // new deployment
        let chain_id: u64 = 5;
        let domain_separator_goerli = DomainSeparator::new(chain_id, contract_address);
        // domain separator is taken from goerli deployment at address 0x9008D19f58AAbD9eD0D60971565AA8510560ab41
        let expected_domain_separator = DomainSeparator(hex!(
            "fb378b35457022ecc5709ae5dafad9393c1387ae6d8ce24913a0c969074c07fb"
        ));
        assert_eq!(domain_separator_goerli, expected_domain_separator);
    }

    #[test]
    fn token_pair_contains() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let token_c = H160::from_low_u64_be(2);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        assert!(pair.contains(&token_a));
        assert!(pair.contains(&token_b));
        assert!(!pair.contains(&token_c));
    }

    #[test]
    fn token_pair_other() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let token_c = H160::from_low_u64_be(2);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        assert_eq!(pair.other(&token_a), Some(token_b));
        assert_eq!(pair.other(&token_b), Some(token_a));
        assert_eq!(pair.other(&token_c), None);
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

    #[test]
    fn token_pair_ordering() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let token_c = H160::from_low_u64_be(2);
        let pair_ab = TokenPair::new(token_a, token_b).unwrap();
        let pair_bc = TokenPair::new(token_b, token_c).unwrap();
        let pair_ca = TokenPair::new(token_c, token_a).unwrap();

        assert_eq!(pair_ab.cmp(&pair_bc), Ordering::Less);
        assert_eq!(pair_ab.cmp(&pair_ca), Ordering::Less);
        assert_eq!(pair_bc.cmp(&pair_ca), Ordering::Greater);
        assert_eq!(pair_ab.cmp(&TokenPair::first_ord()), Ordering::Equal);
    }
}

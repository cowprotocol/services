//! Solana instantiation of the chain vocabulary.
//!
//! No solana-sdk dependency: the algorithm only needs identifiers it can
//! hash and compare, 32-byte newtypes suffice.

use {
    crate::{Amount, ChainTypes, MathError, MathResult},
    std::{error, fmt, str::FromStr},
};

/// A Solana account address (token mint or solver identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pubkey(pub [u8; 32]);

/// A Solana order identifier: the 32-byte intent hash. Unlike the EVM
/// 56-byte UID it carries no embedded owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntentHash(pub [u8; 32]);

/// Application-specific data attached to a Solana order: 32 opaque bytes.
/// Serialized as `0x`-prefixed hex on the wire, just like [`IntentHash`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppData(pub [u8; 32]);

/// `0x`-prefixed hex, the wire and log rendering of an order uid.
impl fmt::Display for IntentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = const_hex::Buffer::<32, true>::new();
        f.write_str(buffer.format(&self.0))
    }
}

impl FromStr for IntentHash {
    type Err = const_hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        const_hex::decode_to_slice(s.strip_prefix("0x").unwrap_or(s), &mut bytes)?;
        Ok(Self(bytes))
    }
}

/// `0x`-prefixed hex, the wire rendering of order `app_data`.
impl fmt::Display for AppData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = const_hex::Buffer::<32, true>::new();
        f.write_str(buffer.format(&self.0))
    }
}

impl FromStr for AppData {
    type Err = const_hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        const_hex::decode_to_slice(s.strip_prefix("0x").unwrap_or(s), &mut bytes)?;
        Ok(Self(bytes))
    }
}

/// Base58, the canonical Solana address rendering.
impl fmt::Display for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&bs58::encode(self.0).into_string())
    }
}

/// A string that does not decode to exactly 32 base58 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPubkey;

impl fmt::Display for InvalidPubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a base58-encoded 32-byte key")
    }
}

impl error::Error for InvalidPubkey {}

impl FromStr for Pubkey {
    type Err = InvalidPubkey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = bs58::decode(s).into_vec().map_err(|_| InvalidPubkey)?;
        Ok(Self(bytes.try_into().map_err(|_| InvalidPubkey)?))
    }
}

/// A Solana transaction signature (64 bytes), rendered as base58.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature(pub [u8; 64]);

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&bs58::encode(self.0).into_string())
    }
}

/// A string that does not decode to exactly 64 base58 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSignature;

impl fmt::Display for InvalidSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a base58-encoded 64-byte signature")
    }
}

impl error::Error for InvalidSignature {}

impl FromStr for Signature {
    type Err = InvalidSignature;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = bs58::decode(s).into_vec().map_err(|_| InvalidSignature)?;
        Ok(Self(bytes.try_into().map_err(|_| InvalidSignature)?))
    }
}

/// The Solana chain marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Solana;

impl ChainTypes for Solana {
    type AccountId = Pubkey;
    type Amount = u64;
    type OrderUid = IntentHash;
    type TokenId = Pubkey;

    /// Lamports per SOL (9 decimals).
    const NATIVE_PRICE_DENOMINATOR: u64 = 1_000_000_000;

    /// Identity: Solana orders name SPL mints directly, there is no
    /// native-token sentinel to map.
    fn canonical_token(token: Pubkey, _wrapped_native: Pubkey) -> Pubkey {
        token
    }

    /// The intent hash embeds no owner, so JIT orders cannot be attributed.
    fn uid_owner(_uid: &IntentHash) -> Option<Pubkey> {
        None
    }
}

/// u64 amounts always use u128 intermediates: every u64 product fits, so
/// unlike the EVM's U256 the non-widening and widening variants coincide.
impl Amount for u64 {
    fn try_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        if div == 0 {
            return Err(MathError::DivisionByZero);
        }
        let wide = u128::from(self) * u128::from(mul) / u128::from(div);
        u64::try_from(wide).map_err(|_| MathError::Overflow)
    }

    fn try_mul_div_ceil(self, mul: Self, div: Self) -> MathResult<Self> {
        if div == 0 {
            return Err(MathError::DivisionByZero);
        }
        let wide = (u128::from(self) * u128::from(mul)).div_ceil(u128::from(div));
        u64::try_from(wide).map_err(|_| MathError::Overflow)
    }

    fn try_widening_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        self.try_mul_div_floor(mul, div)
    }

    fn try_mul_f64(self, factor: f64) -> MathResult<Self> {
        if !factor.is_finite() || factor < 0.0 {
            return Err(MathError::Overflow);
        }
        let result = self as f64 * factor;
        (result <= u64::MAX as f64)
            .then_some(result as u64)
            .ok_or(MathError::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_round_trips_as_0x_hex() {
        let app_data = AppData([0xAB; 32]);
        let encoded = app_data.to_string();
        assert_eq!(
            encoded,
            "0xabababababababababababababababababababababababababababababababab"
        );
        assert_eq!(AppData::from_str(&encoded).unwrap(), app_data);
    }

    #[test]
    fn intent_hash_round_trips_as_0x_hex() {
        let hash = IntentHash([0xCD; 32]);
        let encoded = hash.to_string();
        assert_eq!(
            encoded,
            "0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
        );
        assert_eq!(IntentHash::from_str(&encoded).unwrap(), hash);
    }
}

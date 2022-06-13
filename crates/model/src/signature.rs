use super::DomainSeparator;
use anyhow::{Context as _, Result};
use primitive_types::{H160, H256};
use serde::{de, Deserialize, Serialize};
use std::{convert::TryInto as _, fmt};
use web3::{
    signing::{self, Key, SecretKeyRef},
    types::Recovery,
};

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
}

impl Default for SigningScheme {
    fn default() -> Self {
        SigningScheme::Eip712
    }
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "signingScheme", content = "signature")]
pub enum Signature {
    Eip712(EcdsaSignature),
    EthSign(EcdsaSignature),
    PreSign(H160),
}

impl Default for Signature {
    fn default() -> Self {
        Self::default_with(SigningScheme::default())
    }
}

impl Signature {
    pub fn default_with(scheme: SigningScheme) -> Self {
        match scheme {
            SigningScheme::Eip712 => Signature::Eip712(Default::default()),
            SigningScheme::EthSign => Signature::EthSign(Default::default()),
            SigningScheme::PreSign => Signature::PreSign(Default::default()),
        }
    }

    /// Recovers the owner of the specified signature.
    pub fn recover(
        &self,
        domain_separator: &DomainSeparator,
        struct_hash: &[u8; 32],
    ) -> Option<H160> {
        match self {
            Self::Eip712(signature) => {
                signature.recover(EcdsaSigningScheme::Eip712, domain_separator, struct_hash)
            }
            Self::EthSign(signature) => {
                signature.recover(EcdsaSigningScheme::EthSign, domain_separator, struct_hash)
            }
            Self::PreSign(from) => Some(*from),
        }
    }

    /// Verifies the owner for the specified creation signature.
    pub fn verify_owner(
        &self,
        expected_owner: Option<H160>,
        domain_separator: &DomainSeparator,
        struct_hash: &[u8; 32],
    ) -> Result<H160, VerificationError> {
        let recovered_owner = self
            .recover(domain_separator, struct_hash)
            .ok_or(VerificationError::UnableToRecoverSigner)?;

        if matches!(expected_owner, Some(expected_owner) if recovered_owner != expected_owner) {
            return Err(VerificationError::UnexpectedSigner(recovered_owner));
        }

        Ok(recovered_owner)
    }

    pub fn from_bytes(scheme: SigningScheme, bytes: &[u8]) -> Result<Self> {
        Ok(match scheme {
            scheme @ (SigningScheme::Eip712 | SigningScheme::EthSign) => {
                let bytes: [u8; 65] = bytes
                    .try_into()
                    .context("ECDSA signature must be 65 bytes long")?;
                EcdsaSignature {
                    r: H256::from_slice(&bytes[..32]),
                    s: H256::from_slice(&bytes[32..64]),
                    v: bytes[64],
                }
                .to_signature(
                    scheme
                        .try_to_ecdsa_scheme()
                        .expect("scheme is an ecdsa scheme"),
                )
            }
            SigningScheme::PreSign => Signature::PreSign(H160(
                bytes
                    .try_into()
                    .context("pre-signature must be exactly 20 bytes long")?,
            )),
        })
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Signature::Eip712(sig) | Signature::EthSign(sig) => sig.to_bytes().to_vec(),
            Signature::PreSign(account) => account.0.to_vec(),
        }
    }

    pub fn scheme(&self) -> SigningScheme {
        match self {
            Signature::Eip712(_) => SigningScheme::Eip712,
            Signature::EthSign(_) => SigningScheme::EthSign,
            Signature::PreSign(_) => SigningScheme::PreSign,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationError {
    UnableToRecoverSigner,
    UnexpectedSigner(H160),
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EcdsaSigningScheme {
    Eip712,
    EthSign,
}

impl From<EcdsaSigningScheme> for SigningScheme {
    fn from(scheme: EcdsaSigningScheme) -> Self {
        match scheme {
            EcdsaSigningScheme::Eip712 => Self::Eip712,
            EcdsaSigningScheme::EthSign => Self::EthSign,
        }
    }
}

impl SigningScheme {
    pub fn is_ecdsa_scheme(&self) -> bool {
        self.try_to_ecdsa_scheme().is_some()
    }

    pub fn try_to_ecdsa_scheme(&self) -> Option<EcdsaSigningScheme> {
        match self {
            Self::Eip712 => Some(EcdsaSigningScheme::Eip712),
            Self::EthSign => Some(EcdsaSigningScheme::EthSign),
            Self::PreSign => None,
        }
    }
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Hash)]
pub struct EcdsaSignature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
}

pub fn hashed_eip712_message(
    domain_separator: &DomainSeparator,
    struct_hash: &[u8; 32],
) -> [u8; 32] {
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
    signing_scheme: EcdsaSigningScheme,
    domain_separator: &DomainSeparator,
    struct_hash: &[u8; 32],
) -> [u8; 32] {
    match signing_scheme {
        EcdsaSigningScheme::Eip712 => hashed_eip712_message(domain_separator, struct_hash),
        EcdsaSigningScheme::EthSign => hashed_ethsign_message(domain_separator, struct_hash),
    }
}

impl EcdsaSignature {
    pub fn to_signature(self, scheme: EcdsaSigningScheme) -> Signature {
        match scheme {
            EcdsaSigningScheme::Eip712 => Signature::Eip712(self),
            EcdsaSigningScheme::EthSign => Signature::EthSign(self),
        }
    }

    /// r + s + v
    pub fn to_bytes(self) -> [u8; 65] {
        let mut bytes = [0u8; 65];
        bytes[..32].copy_from_slice(self.r.as_bytes());
        bytes[32..64].copy_from_slice(self.s.as_bytes());
        bytes[64] = self.v;
        bytes
    }

    pub fn from_bytes(bytes: &[u8; 65]) -> Self {
        EcdsaSignature {
            r: H256::from_slice(&bytes[..32]),
            s: H256::from_slice(&bytes[32..64]),
            v: bytes[64],
        }
    }

    pub fn recover(
        &self,
        signing_scheme: EcdsaSigningScheme,
        domain_separator: &DomainSeparator,
        struct_hash: &[u8; 32],
    ) -> Option<H160> {
        let message = hashed_signing_message(signing_scheme, domain_separator, struct_hash);
        let recovery = Recovery::new(message, self.v as u64, self.r, self.s);
        let (signature, recovery_id) = recovery.as_signature()?;
        signing::recover(&message, &signature, recovery_id).ok()
    }

    pub fn sign(
        signing_scheme: EcdsaSigningScheme,
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

    /// Returns an arbitrary non-zero signature that can be used for recovery
    /// when you don't actually care about the owner.
    pub fn non_zero() -> Self {
        Self {
            r: H256([1; 32]),
            s: H256([2; 32]),
            v: 27,
        }
    }
}

impl Serialize for EcdsaSignature {
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

impl<'de> Deserialize<'de> for EcdsaSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor {}
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = EcdsaSignature;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "the 65 ecdsa signature bytes as a hex encoded string, ordered as r, s, v, where v is either 27 or 28"
                )
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let s = s.strip_prefix("0x").ok_or_else(|| {
                    de::Error::custom(format!(
                        "{:?} can't be decoded as hex ecdsa signature because it does not start with '0x'",
                        s
                    ))
                })?;
                let mut bytes = [0u8; 65];
                hex::decode_to_slice(s, &mut bytes).map_err(|err| {
                    de::Error::custom(format!(
                        "failed to decode {:?} as hex ecdsa signature: {}",
                        s, err
                    ))
                })?;
                Ok(EcdsaSignature::from_bytes(&bytes))
            }
        }

        deserializer.deserialize_str(Visitor {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn presign_recovers_to_account() {
        assert_eq!(
            Signature::PreSign(H160([0x42; 20])).recover(&Default::default(), &Default::default()),
            Some(H160([0x42; 20])),
        );
    }

    #[test]
    fn presign_fails_to_convert_to_ecdsa_signature() {
        assert!(SigningScheme::PreSign.try_to_ecdsa_scheme().is_none());
    }

    #[test]
    fn signature_from_bytes() {
        assert_eq!(
            Signature::from_bytes(SigningScheme::Eip712, &[0u8; 20])
                .unwrap_err()
                .to_string(),
            "ECDSA signature must be 65 bytes long"
        );
        assert_eq!(
            Signature::from_bytes(SigningScheme::EthSign, &[0u8; 20])
                .unwrap_err()
                .to_string(),
            "ECDSA signature must be 65 bytes long"
        );
        assert_eq!(
            Signature::from_bytes(SigningScheme::PreSign, &[0u8; 32])
                .unwrap_err()
                .to_string(),
            "pre-signature must be exactly 20 bytes long"
        );

        assert_eq!(
            Signature::from_bytes(SigningScheme::Eip712, &[0u8; 65]).unwrap(),
            Signature::default_with(SigningScheme::Eip712)
        );
        assert_eq!(
            Signature::from_bytes(SigningScheme::EthSign, &[0u8; 65]).unwrap(),
            Signature::default_with(SigningScheme::EthSign)
        );
        assert_eq!(
            Signature::from_bytes(SigningScheme::PreSign, &[0u8; 20]).unwrap(),
            Signature::default_with(SigningScheme::PreSign)
        );
    }

    #[test]
    fn signature_to_bytes() {
        assert_eq!(
            Signature::default_with(SigningScheme::Eip712).to_bytes(),
            [0u8; 65].to_vec()
        );
        assert_eq!(
            Signature::default_with(SigningScheme::EthSign).to_bytes(),
            [0u8; 65].to_vec()
        );
        assert_eq!(
            Signature::default_with(SigningScheme::PreSign).to_bytes(),
            [0u8; 20].to_vec()
        );
        // and something non-trivial
        assert_eq!(
            Signature::from_bytes(SigningScheme::PreSign, &[1u8; 20])
                .unwrap()
                .to_bytes(),
            [1u8; 20].to_vec()
        );
    }

    #[test]
    fn ecdsa_scheme_conversion() {
        for ecdsa_scheme in [EcdsaSigningScheme::Eip712, EcdsaSigningScheme::EthSign] {
            let scheme = SigningScheme::from(ecdsa_scheme);
            assert!(scheme.is_ecdsa_scheme())
        }
        assert!(!SigningScheme::PreSign.is_ecdsa_scheme())
    }

    #[test]
    fn deserialize_and_back() {
        let value = json!(
        {
            "signature": "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "signingScheme": "eip712"
        });
        let expected = Signature::default();
        let deserialized: Signature = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(deserialized, expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(value, serialized);

        assert_eq!(
            serde_json::from_value::<Signature>(json!(
            {
                "signature": "1234",
                "signingScheme": "eip712"
            }))
            .unwrap_err()
            .to_string(),
            "\"1234\" can't be decoded as hex ecdsa signature because it does not start with '0x'"
        );
        assert_eq!(
            serde_json::from_value::<Signature>(json!(
            {
                "signature": "0x42",
                "signingScheme": "eip712"
            }))
            .unwrap_err()
            .to_string(),
            "failed to decode \"42\" as hex ecdsa signature: Invalid string length"
        );
    }
}

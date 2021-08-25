use super::DomainSeparator;
use primitive_types::{H160, H256};
use serde::{de, Deserialize, Serialize};
use std::fmt;
use web3::{
    signing::{self, Key, SecretKeyRef},
    types::Recovery,
};

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
}
#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "signingScheme", content = "signature")]
pub enum Signature {
    Eip712(EcdsaSignature),
    EthSign(EcdsaSignature),
}

impl Default for Signature {
    fn default() -> Self {
        Self::default_with(SigningScheme::Eip712)
    }
}

impl Signature {
    pub fn default_with(scheme: SigningScheme) -> Self {
        match scheme {
            SigningScheme::Eip712 => Signature::Eip712(Default::default()),
            SigningScheme::EthSign => Signature::EthSign(Default::default()),
        }
    }
}

impl Signature {
    pub fn validate(
        &self,
        domain_separator: &DomainSeparator,
        struct_hash: &[u8; 32],
    ) -> Option<H160> {
        match self {
            Signature::Eip712(sig) | Signature::EthSign(sig) => sig.validate(
                self.scheme()
                    .try_to_ecdsa_scheme()
                    .expect("matches an ecdsa scheme"),
                domain_separator,
                struct_hash,
            ),
        }
    }

    pub fn from_bytes(scheme: SigningScheme, bytes: &[u8; 65]) -> Self {
        match scheme {
            scheme @ (SigningScheme::Eip712 | SigningScheme::EthSign) => EcdsaSignature {
                r: H256::from_slice(&bytes[..32]),
                s: H256::from_slice(&bytes[32..64]),
                v: bytes[64],
            }
            .to_signature(
                scheme
                    .try_to_ecdsa_scheme()
                    .expect("scheme is an ecdsa scheme"),
            ),
        }
    }

    pub fn to_bytes(&self) -> [u8; 65] {
        match self {
            Signature::Eip712(sig) | Signature::EthSign(sig) => sig.to_bytes(),
        }
    }

    pub fn scheme(&self) -> SigningScheme {
        match self {
            Signature::Eip712(_) => SigningScheme::Eip712,
            Signature::EthSign(_) => SigningScheme::EthSign,
        }
    }
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
    pub fn try_to_ecdsa_scheme(&self) -> Option<EcdsaSigningScheme> {
        match self {
            Self::Eip712 => Some(EcdsaSigningScheme::Eip712),
            Self::EthSign => Some(EcdsaSigningScheme::EthSign),
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

    pub fn validate(
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

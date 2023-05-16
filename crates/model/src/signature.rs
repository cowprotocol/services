use {
    crate::{bytes_hex, quote::QuoteSigningScheme},
    anyhow::{ensure, Context as _, Result},
    primitive_types::{H160, H256},
    serde::{de, Deserialize, Serialize},
    std::{
        convert::TryInto as _,
        fmt::{self, Debug, Formatter},
    },
    web3::{
        signing::{self, Key, SecretKeyRef},
        types::Recovery,
    },
};

/// See [`Signature`].
#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SigningScheme {
    #[default]
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

impl From<QuoteSigningScheme> for SigningScheme {
    fn from(scheme: QuoteSigningScheme) -> Self {
        match scheme {
            QuoteSigningScheme::Eip712 => SigningScheme::Eip712,
            QuoteSigningScheme::Eip1271 { .. } => SigningScheme::Eip1271,
            QuoteSigningScheme::PreSign { .. } => SigningScheme::PreSign,
            QuoteSigningScheme::EthSign => SigningScheme::EthSign,
        }
    }
}

/// Signature over the order data.
/// All variants rely on the EIP-712 hash of the order data, referred to as the
/// order hash.
#[derive(Eq, PartialEq, Clone, Deserialize, Serialize, Hash)]
#[serde(into = "JsonSignature", try_from = "JsonSignature")]
pub enum Signature {
    /// The order struct is signed according to EIP-712.
    ///
    /// https://eips.ethereum.org/EIPS/eip-712
    Eip712(EcdsaSignature),
    /// The order hash is signed according to EIP-191's personal_sign signature
    /// format.
    ///
    /// https://eips.ethereum.org/EIPS/eip-191
    EthSign(EcdsaSignature),
    /// Signature verified according to EIP-1271, which facilitates a way for
    /// contracts to verify signatures using an arbitrary method. This
    /// allows smart contracts to sign and place orders. The order hash is
    /// passed to the verification method, along with this signature.
    ///
    /// https://eips.ethereum.org/EIPS/eip-1271
    Eip1271(Vec<u8>),
    /// For these signatures, the user broadcasts a transaction onchain. This
    /// transaction contains a signature of the order hash. Because this
    /// onchain transaction is also signed, it proves that the user indeed
    /// signed the order.
    PreSign,
}

impl Default for Signature {
    fn default() -> Self {
        Self::default_with(SigningScheme::default())
    }
}

impl Debug for Signature {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if let Signature::PreSign = self {
            return f.write_str("PreSign");
        }

        let scheme = format!("{:?}", self.scheme());
        let bytes = format!("0x{}", hex::encode(self.to_bytes()));
        f.debug_tuple(&scheme).field(&bytes).finish()
    }
}

impl Signature {
    pub fn default_with(scheme: SigningScheme) -> Self {
        match scheme {
            SigningScheme::Eip712 => Signature::Eip712(Default::default()),
            SigningScheme::EthSign => Signature::EthSign(Default::default()),
            SigningScheme::Eip1271 => Signature::Eip1271(Default::default()),
            SigningScheme::PreSign => Signature::PreSign,
        }
    }

    /// Recovers the owner of the specified signature for a given message.
    ///
    /// This method returns an error if there is an issue recovering an ECDSA
    /// signature, or `None` for on-chain schemes that don't support owner
    /// recovery.
    pub fn recover(&self, message: &[u8; 32]) -> Result<Option<H160>> {
        match self {
            Self::Eip712(signature) => signature
                .recover(EcdsaSigningScheme::Eip712, message)
                .map(Some),
            Self::EthSign(signature) => signature
                .recover(EcdsaSigningScheme::EthSign, message)
                .map(Some),
            _ => Ok(None),
        }
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
            SigningScheme::Eip1271 => Self::Eip1271(bytes.to_vec()),
            SigningScheme::PreSign => {
                ensure!(
                    bytes.is_empty() || bytes.len() == 20,
                    "presign signature bytes should be empty or an address (legacy)",
                );
                Self::PreSign
            }
        })
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Eip712(signature) | Self::EthSign(signature) => signature.to_bytes().to_vec(),
            Self::Eip1271(signature) => signature.clone(),
            Self::PreSign => Vec::new(),
        }
    }

    pub fn scheme(&self) -> SigningScheme {
        match self {
            Signature::Eip712(_) => SigningScheme::Eip712,
            Signature::EthSign(_) => SigningScheme::EthSign,
            Signature::Eip1271(_) => SigningScheme::Eip1271,
            Signature::PreSign => SigningScheme::PreSign,
        }
    }

    pub fn encode_for_settlement(&self, owner: H160) -> Vec<u8> {
        match self {
            Self::Eip712(signature) | Self::EthSign(signature) => signature.to_bytes().to_vec(),
            Self::Eip1271(signature) => [owner.as_bytes(), signature].concat(),
            Self::PreSign => owner.as_bytes().to_vec(),
        }
    }
}

/// An internal type used for deriving `serde` implementations for the
/// `Signature` type.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSignature {
    signing_scheme: SigningScheme,
    #[serde(with = "bytes_hex")]
    signature: Vec<u8>,
}

impl From<Signature> for JsonSignature {
    fn from(signature: Signature) -> Self {
        Self {
            signing_scheme: signature.scheme(),
            signature: signature.to_bytes(),
        }
    }
}

impl TryFrom<JsonSignature> for Signature {
    type Error = anyhow::Error;

    fn try_from(json: JsonSignature) -> Result<Self, Self::Error> {
        Self::from_bytes(json.signing_scheme, &json.signature)
    }
}

#[derive(Debug)]
pub struct InvalidSignature;

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
            Self::Eip1271 | Self::PreSign => None,
        }
    }
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Hash)]
pub struct EcdsaSignature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
}

/// Returns the message used for signing and recovery for the specified hash.
///
/// The signing message depends on the signature scheme that was used.
fn signing_message(signing_scheme: EcdsaSigningScheme, hash: &[u8; 32]) -> [u8; 32] {
    match signing_scheme {
        EcdsaSigningScheme::Eip712 => *hash,
        EcdsaSigningScheme::EthSign => {
            let mut buffer = [0u8; 60];
            buffer[..28].copy_from_slice(b"\x19Ethereum Signed Message:\n32");
            buffer[28..].copy_from_slice(hash);
            signing::keccak256(&buffer)
        }
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

    pub fn recover(&self, signing_scheme: EcdsaSigningScheme, hash: &[u8; 32]) -> Result<H160> {
        let message = signing_message(signing_scheme, hash);
        let recovery = Recovery::new(message, self.v as u64, self.r, self.s);
        let (signature, recovery_id) = recovery
            .as_signature()
            .context("unexpectedly invalid signature")?;
        Ok(signing::recover(&message, &signature, recovery_id)?)
    }

    pub fn sign(signing_scheme: EcdsaSigningScheme, hash: &[u8; 32], key: SecretKeyRef) -> Self {
        let message = signing_message(signing_scheme, hash);
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
                    "the 65 ecdsa signature bytes as a hex encoded string, ordered as r, s, v, \
                     where v is either 27 or 28"
                )
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let s = s.strip_prefix("0x").ok_or_else(|| {
                    de::Error::custom(format!(
                        "{s:?} can't be decoded as hex ecdsa signature because it does not start \
                         with '0x'"
                    ))
                })?;
                let mut bytes = [0u8; 65];
                hex::decode_to_slice(s, &mut bytes).map_err(|err| {
                    de::Error::custom(format!(
                        "failed to decode {s:?} as hex ecdsa signature: {err}"
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
    use {super::*, serde_json::json};

    #[test]
    fn onchain_signatures_cannot_recover_owners() {
        for signature in [Signature::PreSign, Signature::Eip1271(Default::default())] {
            assert_eq!(signature.recover(&Default::default()).unwrap(), None);
        }
    }

    #[test]
    fn onchain_signatures_fail_to_convert_to_ecdsa_signature() {
        for signature in [SigningScheme::PreSign, SigningScheme::Eip1271] {
            assert!(signature.try_to_ecdsa_scheme().is_none());
        }
    }

    #[test]
    fn signature_from_bytes() {
        assert!(Signature::from_bytes(SigningScheme::Eip712, &[0u8; 20]).is_err());
        assert!(Signature::from_bytes(SigningScheme::EthSign, &[0u8; 20]).is_err());
        assert!(Signature::from_bytes(SigningScheme::PreSign, &[0u8; 32]).is_err());

        assert_eq!(
            Signature::from_bytes(SigningScheme::Eip712, &[0u8; 65]).unwrap(),
            Signature::default_with(SigningScheme::Eip712)
        );
        assert_eq!(
            Signature::from_bytes(SigningScheme::EthSign, &[0u8; 65]).unwrap(),
            Signature::default_with(SigningScheme::EthSign)
        );
        assert_eq!(
            Signature::from_bytes(SigningScheme::PreSign, &[]).unwrap(),
            Signature::default_with(SigningScheme::PreSign)
        );
        assert_eq!(
            Signature::from_bytes(SigningScheme::Eip1271, &[]).unwrap(),
            Signature::default_with(SigningScheme::Eip1271)
        );
        assert_eq!(
            Signature::from_bytes(SigningScheme::Eip1271, &[1, 2, 3]).unwrap(),
            Signature::Eip1271(vec![1, 2, 3]),
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
            Vec::<u8>::new()
        );
        assert_eq!(Signature::Eip1271(vec![1, 2, 3]).to_bytes(), vec![1, 2, 3]);
    }

    #[test]
    fn ecdsa_scheme_conversion() {
        for ecdsa_scheme in [EcdsaSigningScheme::Eip712, EcdsaSigningScheme::EthSign] {
            let scheme = SigningScheme::from(ecdsa_scheme);
            assert!(scheme.is_ecdsa_scheme())
        }

        for onchain_scheme in [SigningScheme::PreSign, SigningScheme::Eip1271] {
            assert!(!onchain_scheme.is_ecdsa_scheme())
        }
    }

    #[test]
    fn deserialize_and_back() {
        for (signature, json) in [
            (
                Signature::Eip712(Default::default()),
                json!({
                    "signingScheme": "eip712",
                    "signature": "0x\
                        0000000000000000000000000000000000000000000000000000000000000000\
                        0000000000000000000000000000000000000000000000000000000000000000\
                        00",
                }),
            ),
            (
                Signature::EthSign(EcdsaSignature {
                    r: H256([1; 32]),
                    s: H256([2; 32]),
                    v: 3,
                }),
                json!({
                    "signingScheme": "ethsign",
                    "signature": "0x\
                        0101010101010101010101010101010101010101010101010101010101010101\
                        0202020202020202020202020202020202020202020202020202020202020202\
                        03",
                }),
            ),
            (
                Signature::Eip1271(vec![1, 2, 3]),
                json!({
                    "signingScheme": "eip1271",
                    "signature": "0x010203",
                }),
            ),
            (
                Signature::Eip1271(Default::default()),
                json!({
                    "signingScheme": "eip1271",
                    "signature": "0x",
                }),
            ),
            (
                Signature::PreSign,
                json!({
                    "signingScheme": "presign",
                    "signature": "0x",
                }),
            ),
        ] {
            assert_eq!(signature, serde_json::from_value(json.clone()).unwrap());
            assert_eq!(json, json!(signature));
        }
    }

    #[test]
    fn deserialization_errors() {
        for json in [
            json!({
                "signingScheme": "eip712",
                "signature": "0x0102",
            }),
            json!({
                "signingScheme": "ethsign",
                "signature": 1234,
            }),
            json!({
                "signingScheme": "eip1271",
            }),
            json!({
                "signingScheme": "presign",
                "signature": "0x01",
            }),
        ] {
            assert!(serde_json::from_value::<SigningScheme>(json).is_err());
        }
    }

    #[test]
    fn legacy_presign_signature_format() {
        assert_eq!(
            Signature::PreSign,
            Signature::from_bytes(SigningScheme::PreSign, &[0u8; 20]).unwrap(),
        );

        assert_eq!(
            Signature::PreSign,
            serde_json::from_value(json!({
                "signingScheme": "presign",
                "signature": "0x0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f",
            }))
            .unwrap(),
        );
    }
}

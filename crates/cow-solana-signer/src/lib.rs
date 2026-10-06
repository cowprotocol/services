//! Solana signers: a local keypair or an AWS KMS Ed25519 key.

use {
    serde::Deserialize,
    solana_sdk::{
        message::VersionedMessage,
        pubkey::Pubkey,
        signature::Signature,
        signer::{Signer as _, keypair::Keypair},
        transaction::VersionedTransaction,
    },
    std::path::PathBuf,
    thiserror::Error,
};

/// A signer backend named in a config. A config names exactly one.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Config {
    /// Path to a keypair file.
    /// TODO: plaintext keypair paths are temporary, prefer `kms-key`.
    Keypair(PathBuf),
    /// Id, alias, or ARN of an AWS KMS Ed25519 key. The private key never
    /// leaves KMS.
    KmsKey(String),
}

impl Config {
    /// Load the signer the config names.
    pub async fn load(&self) -> Result<Signer, Error> {
        match self {
            Self::Keypair(path) => solana_sdk::signer::keypair::read_keypair_file(path)
                .map(Signer::Keypair)
                .map_err(|error| Error::Keypair {
                    path: path.clone(),
                    error: error.to_string(),
                }),
            Self::KmsKey(key_id) => KmsSigner::new(key_id.clone()).await.map(Signer::Kms),
        }
    }
}

/// Signs with one key.
#[derive(Debug)]
pub enum Signer {
    Keypair(Keypair),
    Kms(KmsSigner),
}

impl Signer {
    /// The signer's on-chain identity.
    pub fn pubkey(&self) -> Pubkey {
        match self {
            Self::Keypair(keypair) => keypair.pubkey(),
            Self::Kms(kms) => kms.pubkey,
        }
    }

    /// Sign a serialized message. The signature is verified locally, so a
    /// misconfigured signer fails here instead of at broadcast.
    pub async fn sign_message(&self, message: &[u8]) -> Result<Signature, Error> {
        let signature = match self {
            Self::Keypair(keypair) => keypair.sign_message(message),
            Self::Kms(kms) => kms.sign(message).await?,
        };
        let pubkey = self.pubkey();
        if !signature.verify(pubkey.as_ref(), message) {
            return Err(Error::InvalidSignature { pubkey });
        }
        Ok(signature)
    }

    /// Sign the message into a submittable transaction. The signer's key is
    /// the only one available, so the message must name it as its sole
    /// signer.
    pub async fn sign(&self, message: VersionedMessage) -> Result<VersionedTransaction, Error> {
        let pubkey = self.pubkey();
        let required = usize::from(message.header().num_required_signatures);
        if required != 1 {
            return Err(Error::MissingSignatures { required });
        }
        if message.static_account_keys().first() != Some(&pubkey) {
            return Err(Error::NotASigner { pubkey });
        }
        let signature = self.sign_message(&message.serialize()).await?;
        Ok(VersionedTransaction {
            signatures: vec![signature],
            message,
        })
    }
}

/// A signer backed by an AWS KMS Ed25519 key: signing is remote, the private
/// key never leaves KMS.
#[derive(Debug)]
pub struct KmsSigner {
    client: aws_sdk_kms::Client,
    key_id: String,
    pubkey: Pubkey,
}

impl KmsSigner {
    /// Build the KMS client from the ambient AWS environment and resolve the
    /// key's public half.
    pub async fn new(key_id: String) -> Result<Self, Error> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = aws_sdk_kms::Client::new(&config);
        let response = client
            .get_public_key()
            .key_id(&key_id)
            .send()
            .await
            .map_err(|error| Error::Kms {
                key_id: key_id.clone(),
                error: aws_sdk_kms::error::DisplayErrorContext(error).to_string(),
            })?;
        let der = response.public_key().ok_or_else(|| Error::NotEd25519 {
            key_id: key_id.clone(),
        })?;
        let pubkey = ed25519_spki_pubkey(der.as_ref()).ok_or_else(|| Error::NotEd25519 {
            key_id: key_id.clone(),
        })?;
        Ok(Self {
            client,
            key_id,
            pubkey,
        })
    }

    async fn sign(&self, message: &[u8]) -> Result<Signature, Error> {
        let kms_error = |error: String| Error::Kms {
            key_id: self.key_id.clone(),
            error,
        };
        let response = self
            .client
            .sign()
            .key_id(&self.key_id)
            .message(aws_sdk_kms::primitives::Blob::new(message))
            .message_type(aws_sdk_kms::types::MessageType::Raw)
            // Pure Ed25519 over the raw message: the prehashed variant
            // produces signatures the chain rejects.
            .signing_algorithm(aws_sdk_kms::types::SigningAlgorithmSpec::Ed25519Sha512)
            .send()
            .await
            .map_err(|error| {
                kms_error(aws_sdk_kms::error::DisplayErrorContext(error).to_string())
            })?;
        let bytes = response
            .signature()
            .ok_or_else(|| kms_error("the response carries no signature".to_owned()))?;
        Signature::try_from(bytes.as_ref()).map_err(|_| kms_error("malformed signature".to_owned()))
    }
}

/// The raw 32-byte key inside an Ed25519 SubjectPublicKeyInfo, the DER shape
/// KMS answers public keys in.
fn ed25519_spki_pubkey(der: &[u8]) -> Option<Pubkey> {
    const HEADER: [u8; 12] = [
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
    ];
    let key: [u8; 32] = der.strip_prefix(HEADER.as_slice())?.try_into().ok()?;
    Some(Pubkey::new_from_array(key))
}

#[derive(Debug, PartialEq, Error)]
pub enum Error {
    #[error("failed to read the keypair at {}: {error}", path.display())]
    Keypair { path: PathBuf, error: String },
    #[error("KMS request for key {key_id} failed: {error}")]
    Kms { key_id: String, error: String },
    #[error("KMS key {key_id} is not an Ed25519 signing key")]
    NotEd25519 { key_id: String },
    #[error("the signer for {pubkey} produced an invalid signature")]
    InvalidSignature { pubkey: Pubkey },
    #[error("{pubkey} is not the message's signer")]
    NotASigner { pubkey: Pubkey },
    #[error("the message requires {required} signers, the signer provides one")]
    MissingSignatures { required: usize },
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_sdk::{hash::Hash, message::v0},
        solana_testlib::temp_keypair,
        std::path::Path,
    };

    fn message(payer: &Pubkey) -> v0::Message {
        let instruction =
            solana_system_interface::instruction::transfer(payer, &Pubkey::new_unique(), 1);
        v0::Message::try_compile(payer, &[instruction], &[], Hash::new_unique()).unwrap()
    }

    /// Each backend parses from its own key. Naming both is a parse error.
    #[test]
    fn parses_one_backend() {
        #[derive(Deserialize)]
        struct Wrapper {
            signer: Config,
        }
        let parse = |toml: &str| toml::from_str::<Wrapper>(toml).map(|wrapper| wrapper.signer);
        assert!(matches!(
            parse(r#"signer = { keypair = "/path/to/keypair.json" }"#),
            Ok(Config::Keypair(path)) if path == Path::new("/path/to/keypair.json")
        ));
        assert!(matches!(
            parse(r#"signer = { kms-key = "arn:aws:kms:eu-central-1:1:key/2" }"#),
            Ok(Config::KmsKey(key)) if key == "arn:aws:kms:eu-central-1:1:key/2"
        ));
        parse(r#"signer = { keypair = "/path/to/keypair.json", kms-key = "arn" }"#).unwrap_err();
    }

    /// The keypair backend loads from its file. A missing file names the
    /// path.
    #[tokio::test]
    async fn loads_a_keypair_file() {
        let file = temp_keypair();
        let signer = Config::Keypair(file.path().to_path_buf()).load().await;
        assert!(matches!(signer, Ok(Signer::Keypair(_))));
        let missing = PathBuf::from("/nonexistent/keypair.json");
        assert!(matches!(
            Config::Keypair(missing.clone()).load().await,
            Err(Error::Keypair { path, .. }) if path == missing
        ));
    }

    /// The keypair backend assembles a transaction whose signature verifies
    /// against the serialized message.
    #[tokio::test]
    async fn keypair_signs_a_verifiable_transaction() {
        let signer = Signer::Keypair(Keypair::new());
        let message = message(&signer.pubkey());
        let transaction = signer.sign(VersionedMessage::V0(message)).await.unwrap();
        let serialized = transaction.message.serialize();
        assert!(
            transaction.signatures[0].verify(signer.pubkey().as_ref(), &serialized),
            "the fee payer slot must carry a valid signature"
        );
    }

    /// A message whose signer is not the signer's key is refused.
    #[tokio::test]
    async fn refuses_a_message_without_the_signer() {
        let signer = Signer::Keypair(Keypair::new());
        let message = message(&Pubkey::new_unique());
        assert!(matches!(
            signer.sign(VersionedMessage::V0(message)).await,
            Err(Error::NotASigner { .. })
        ));
    }

    /// A message needing signatures beyond the signer's own is refused
    /// instead of assembled with empty slots.
    #[tokio::test]
    async fn refuses_a_message_with_unprovidable_signers() {
        let signer = Signer::Keypair(Keypair::new());
        let instruction = solana_system_interface::instruction::transfer(
            &Pubkey::new_unique(),
            &Pubkey::new_unique(),
            1,
        );
        let message =
            v0::Message::try_compile(&signer.pubkey(), &[instruction], &[], Hash::new_unique())
                .unwrap();
        assert_eq!(
            signer.sign(VersionedMessage::V0(message)).await,
            Err(Error::MissingSignatures { required: 2 })
        );
    }

    /// The KMS public key DER unwraps to the raw 32 bytes.
    #[test]
    fn unwraps_the_ed25519_spki() {
        let key = [0x42; 32];
        let mut der = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        der.extend_from_slice(&key);
        assert_eq!(ed25519_spki_pubkey(&der), Some(Pubkey::new_from_array(key)));
        assert_eq!(ed25519_spki_pubkey(&der[1..]), None);
        assert_eq!(ed25519_spki_pubkey(&der[..43]), None);
    }
}

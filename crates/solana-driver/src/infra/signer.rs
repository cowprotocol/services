//! Settlement signers: a local keypair or an AWS KMS Ed25519 key.

use {
    solana_sdk::{
        message::{VersionedMessage, v0},
        pubkey::Pubkey,
        signature::Signature,
        signer::{Signer as _, keypair::Keypair},
        transaction::VersionedTransaction,
    },
    std::sync::Arc,
    thiserror::Error,
};

/// Signs settlement transactions for one solver.
#[derive(Debug)]
pub enum Signer {
    Keypair(Arc<Keypair>),
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

    /// Sign the message into a submittable transaction. Both backends
    /// assemble the transaction the same way, and the signature is verified
    /// locally, so a misconfigured signer fails here instead of at broadcast.
    pub async fn sign(&self, message: v0::Message) -> Result<VersionedTransaction, Error> {
        let message = VersionedMessage::V0(message);
        let bytes = message.serialize();
        let signature = match self {
            Self::Keypair(keypair) => keypair.sign_message(&bytes),
            Self::Kms(kms) => kms.sign(&bytes).await?,
        };
        let pubkey = self.pubkey();
        if !signature.verify(pubkey.as_ref(), &bytes) {
            return Err(Error::InvalidSignature { pubkey });
        }
        let signers = usize::from(message.header().num_required_signatures);
        let index = message
            .static_account_keys()
            .iter()
            .take(signers)
            .position(|key| *key == pubkey)
            .ok_or(Error::NotASigner { pubkey })?;
        let mut signatures = vec![Signature::default(); signers];
        signatures[index] = signature;
        // A settlement carries no co-signers: an empty slot would only fail
        // at broadcast, so refuse the message here.
        if signatures.iter().any(|slot| *slot == Signature::default()) {
            return Err(Error::MissingSignatures { required: signers });
        }
        Ok(VersionedTransaction {
            signatures,
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("KMS request for key {key_id} failed: {error}")]
    Kms { key_id: String, error: String },
    #[error("KMS key {key_id} is not an Ed25519 signing key")]
    NotEd25519 { key_id: String },
    #[error("the signer for {pubkey} produced an invalid signature")]
    InvalidSignature { pubkey: Pubkey },
    #[error("{pubkey} is not among the message's required signers")]
    NotASigner { pubkey: Pubkey },
    #[error("the message requires {required} signers, only the solver's own is available")]
    MissingSignatures { required: usize },
}

#[cfg(test)]
mod tests {
    use {super::*, solana_sdk::hash::Hash};

    fn message(payer: &Pubkey) -> v0::Message {
        let instruction =
            solana_system_interface::instruction::transfer(payer, &Pubkey::new_unique(), 1);
        v0::Message::try_compile(payer, &[instruction], &[], Hash::new_unique()).unwrap()
    }

    /// The keypair backend assembles a transaction whose signature verifies
    /// against the serialized message.
    #[tokio::test]
    async fn keypair_signs_a_verifiable_transaction() {
        let keypair = Keypair::new();
        let signer = Signer::Keypair(Arc::new(keypair));
        let message = message(&signer.pubkey());
        let transaction = signer.sign(message).await.unwrap();
        let serialized = transaction.message.serialize();
        assert!(
            transaction.signatures[0].verify(signer.pubkey().as_ref(), &serialized),
            "the fee payer slot must carry a valid signature"
        );
    }

    /// A message whose signers do not include the signer's key is refused.
    #[tokio::test]
    async fn refuses_a_message_without_the_signer() {
        let signer = Signer::Keypair(Arc::new(Keypair::new()));
        let message = message(&Pubkey::new_unique());
        assert!(matches!(
            signer.sign(message).await,
            Err(Error::NotASigner { .. })
        ));
    }

    /// A message needing signatures beyond the signer's own is refused
    /// instead of assembled with empty slots.
    #[tokio::test]
    async fn refuses_a_message_with_unprovidable_signers() {
        let signer = Signer::Keypair(Arc::new(Keypair::new()));
        let instruction = solana_system_interface::instruction::transfer(
            &Pubkey::new_unique(),
            &Pubkey::new_unique(),
            1,
        );
        let message =
            v0::Message::try_compile(&signer.pubkey(), &[instruction], &[], Hash::new_unique())
                .unwrap();
        assert!(matches!(
            signer.sign(message).await,
            Err(Error::MissingSignatures { required: 2 })
        ));
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

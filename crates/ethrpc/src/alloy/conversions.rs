use {
    alloy::{network::TxSigner, primitives::map::HashMap, signers::Signature},
    anyhow::Context,
};

/////////////////////////////////
// Conversions to the alloy types
/////////////////////////////////

pub trait IntoAlloy {
    /// The corresponding Alloy type.
    type To;

    /// Converts the legacy type to the corresponding Alloy type.
    fn into_alloy(self) -> Self::To;
}

impl IntoAlloy for primitive_types::U256 {
    type To = alloy::primitives::U256;

    fn into_alloy(self) -> Self::To {
        let mut buf = [0u8; 32];
        self.to_little_endian(&mut buf);
        alloy::primitives::U256::from_le_bytes(buf)
    }
}

impl IntoAlloy for primitive_types::U512 {
    type To = alloy::primitives::U512;

    fn into_alloy(self) -> Self::To {
        let mut buf = [0u8; 64];
        self.to_little_endian(&mut buf);
        alloy::primitives::U512::from_le_bytes(buf)
    }
}

impl IntoAlloy for primitive_types::H160 {
    type To = alloy::primitives::Address;

    fn into_alloy(self) -> Self::To {
        alloy::primitives::Address(self.0.into())
    }
}

impl IntoAlloy for primitive_types::H256 {
    type To = alloy::primitives::aliases::B256;

    fn into_alloy(self) -> Self::To {
        alloy::primitives::aliases::B256::new(self.0)
    }
}

impl IntoAlloy for web3::types::BlockNumber {
    type To = alloy::eips::BlockNumberOrTag;

    fn into_alloy(self) -> Self::To {
        match self {
            web3::types::BlockNumber::Finalized => alloy::eips::BlockNumberOrTag::Finalized,
            web3::types::BlockNumber::Safe => alloy::eips::BlockNumberOrTag::Safe,
            web3::types::BlockNumber::Latest => alloy::eips::BlockNumberOrTag::Latest,
            web3::types::BlockNumber::Earliest => alloy::eips::BlockNumberOrTag::Earliest,
            web3::types::BlockNumber::Pending => alloy::eips::BlockNumberOrTag::Pending,
            web3::types::BlockNumber::Number(number) => {
                alloy::eips::BlockNumberOrTag::Number(number.as_u64())
            }
        }
    }
}

impl IntoAlloy for web3::types::BlockId {
    type To = alloy::eips::BlockId;

    fn into_alloy(self) -> Self::To {
        match self {
            web3::types::BlockId::Hash(hash) => {
                alloy::eips::BlockId::Hash(alloy::eips::RpcBlockHash::from(hash.into_alloy()))
            }
            web3::types::BlockId::Number(number) => {
                alloy::eips::BlockId::Number(number.into_alloy())
            }
        }
    }
}

impl IntoAlloy for ethcontract::tokens::Bytes<Vec<u8>> {
    type To = alloy::primitives::Bytes;

    fn into_alloy(self) -> Self::To {
        alloy::primitives::Bytes::copy_from_slice(self.0.as_slice())
    }
}

impl IntoAlloy for web3::types::Bytes {
    type To = alloy::primitives::Bytes;

    fn into_alloy(self) -> Self::To {
        alloy::primitives::Bytes::copy_from_slice(self.0.as_slice())
    }
}

impl IntoAlloy for HashMap<ethcontract::H256, ethcontract::H256> {
    type To = HashMap<
        alloy::primitives::B256,
        alloy::primitives::B256,
        alloy::primitives::map::FbBuildHasher<32>,
    >;

    fn into_alloy(self) -> Self::To {
        self.into_iter()
            .map(|(k, v)| (k.into_alloy(), v.into_alloy()))
            .collect()
    }
}

impl IntoAlloy for ethcontract::state_overrides::StateOverride {
    type To = alloy::rpc::types::eth::state::AccountOverride;

    fn into_alloy(self) -> Self::To {
        Self::To {
            balance: self.balance.map(IntoAlloy::into_alloy),
            nonce: self.nonce.map(|u| u.as_u64()),
            code: self.code.map(IntoAlloy::into_alloy),
            state: self.state.map(IntoAlloy::into_alloy),
            state_diff: self.state_diff.map(IntoAlloy::into_alloy),
            move_precompile_to: None,
        }
    }
}

impl IntoAlloy for ethcontract::state_overrides::StateOverrides {
    type To = alloy::rpc::types::eth::state::StateOverride;

    fn into_alloy(self) -> Self::To {
        alloy::rpc::types::eth::state::StateOverridesBuilder::new(
            self.into_iter()
                .map(|(k, v)| (k.into_alloy(), v.into_alloy()))
                .collect(),
        )
        .build()
    }
}

pub enum Account {
    Address(alloy::primitives::Address),
    Signer(Box<dyn TxSigner<Signature> + Send + Sync + 'static>),
}

#[async_trait::async_trait]
pub trait TryIntoAlloyAsync {
    type Into;

    async fn try_into_alloy(self) -> anyhow::Result<Self::Into>;
}

#[async_trait::async_trait]
impl TryIntoAlloyAsync for ethcontract::Account {
    type Into = Account;

    async fn try_into_alloy(self) -> anyhow::Result<Self::Into> {
        match self {
            ethcontract::Account::Offline(pk, _) => {
                let signer =
                    alloy::signers::local::PrivateKeySigner::from_slice(&pk.secret_bytes())
                        .context("invalid private key bytes")?;
                Ok(Account::Signer(Box::new(signer)))
            }
            ethcontract::Account::Kms(account, chain_id) => {
                let signer = alloy::signers::aws::AwsSigner::new(
                    account.client().clone(),
                    account.key_id().to_string(),
                    chain_id,
                )
                .await?;
                Ok(Account::Signer(Box::new(signer)))
            }
            ethcontract::Account::Local(address, _) => Ok(Account::Address(address.into_alloy())),
            ethcontract::Account::Locked(_, _, _) => {
                anyhow::bail!("Locked accounts are not currently supported")
            }
        }
    }
}

//////////////////////////////////
// Conversions to the legacy types
//////////////////////////////////

pub trait IntoLegacy {
    /// The corresponding legacy type.
    type To;

    /// Converts the alloy type to the corresponding legacy type.
    fn into_legacy(self) -> Self::To;
}

impl IntoLegacy for alloy::primitives::U256 {
    type To = primitive_types::U256;

    fn into_legacy(self) -> Self::To {
        primitive_types::U256(self.into_limbs())
    }
}

impl IntoLegacy for alloy::primitives::U512 {
    type To = primitive_types::U512;

    fn into_legacy(self) -> Self::To {
        primitive_types::U512(self.into_limbs())
    }
}

impl IntoLegacy for alloy::primitives::Address {
    type To = primitive_types::H160;

    fn into_legacy(self) -> Self::To {
        primitive_types::H160(self.into())
    }
}

impl IntoLegacy for alloy::primitives::aliases::B256 {
    type To = primitive_types::H256;

    fn into_legacy(self) -> Self::To {
        primitive_types::H256(self.into())
    }
}

impl IntoLegacy for alloy::primitives::Bytes {
    type To = web3::types::Bytes;

    fn into_legacy(self) -> Self::To {
        web3::types::Bytes(self.to_vec())
    }
}

impl IntoLegacy
    for HashMap<
        alloy::primitives::B256,
        alloy::primitives::B256,
        alloy::primitives::map::FbBuildHasher<32>,
    >
{
    type To = HashMap<ethcontract::H256, ethcontract::H256>;

    fn into_legacy(self) -> Self::To {
        self.into_iter()
            .map(|(k, v)| (k.into_legacy(), v.into_legacy()))
            .collect()
    }
}

impl IntoLegacy for alloy::rpc::types::eth::state::AccountOverride {
    type To = ethcontract::state_overrides::StateOverride;

    fn into_legacy(self) -> Self::To {
        Self::To {
            balance: self.balance.map(IntoLegacy::into_legacy),
            nonce: self.nonce.map(Into::into),
            code: self.code.map(IntoLegacy::into_legacy),
            state: self.state.map(IntoLegacy::into_legacy),
            state_diff: self.state_diff.map(IntoLegacy::into_legacy),
        }
    }
}

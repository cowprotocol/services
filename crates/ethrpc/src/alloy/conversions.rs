use std::collections::HashMap;

/////////////////////////////////
// Conversions to the alloy types
/////////////////////////////////

pub trait IntoAlloy {
    /// The corresponding Alloy type.
    type To;

    /// Converts the legacy type to the corresponding Alloy type.
    fn into_alloy(self) -> Self::To;
}

impl IntoAlloy for ethcontract::I256 {
    type To = alloy::primitives::I256;

    fn into_alloy(self) -> Self::To {
        let mut buf = [0u8; 32];
        self.to_little_endian(&mut buf);
        alloy::primitives::I256::from_le_bytes(buf)
    }
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

use {
    alloy::{
        primitives::{B256, Bytes},
        rpc::types::TransactionRequest,
    },
    derive_more::derive::{From, Into},
    eth_domain_types::{AccessList, Address, BlockNo, Ether},
};

/// A transaction ID, AKA transaction hash.
#[derive(Clone, Debug, From, Into)]
pub struct TxId(pub B256);

pub enum TxStatus {
    /// The transaction has been included and executed successfully.
    Executed { block_number: BlockNo },
    /// The transaction has been included but execution failed.
    Reverted { block_number: BlockNo },
    /// The transaction has not been included yet.
    Pending,
}

/// An onchain transaction.
#[derive(derive_more::Debug, Clone)]
pub struct Tx {
    pub from: Address,
    pub to: Address,
    pub value: Ether,
    pub input: Bytes,
    #[debug(ignore)]
    pub access_list: AccessList,
}

impl From<Tx> for TransactionRequest {
    fn from(value: Tx) -> Self {
        TransactionRequest::default()
            .from(value.from)
            .to(value.to)
            .value(value.value.0)
            .input(value.input.into())
            .access_list(value.access_list.into())
    }
}

impl Tx {
    pub fn set_access_list(self, access_list: AccessList) -> Self {
        Self {
            access_list,
            ..self
        }
    }
}

/// The Keccak-256 hash of a contract's initialization code.
///
/// This value is meaningful in the context of the EVM `CREATE2` opcode in that
/// it influences the deterministic address that the contract ends up on.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CodeDigest(pub B256);

impl From<B256> for CodeDigest {
    fn from(value: B256) -> Self {
        Self(value)
    }
}

impl From<CodeDigest> for B256 {
    fn from(value: CodeDigest) -> Self {
        value.0
    }
}

impl From<[u8; 32]> for CodeDigest {
    fn from(value: [u8; 32]) -> Self {
        Self(B256::from(value))
    }
}

impl From<CodeDigest> for [u8; 32] {
    fn from(value: CodeDigest) -> Self {
        value.0.into()
    }
}

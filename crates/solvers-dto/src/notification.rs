use {
    alloy::{
        primitives::{Address, B256, U256},
        rpc::types::AccessList,
    },
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    std::collections::BTreeSet,
};

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub auction_id: Option<i64>,
    pub solution_id: Option<SolutionId>,
    #[serde(flatten)]
    pub kind: Kind,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SolutionId {
    Single(u64),
    Merged(Vec<u64>),
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Kind {
    Timeout,
    EmptySolution,
    DuplicatedSolutionId,
    #[serde(rename_all = "camelCase")]
    SimulationFailed {
        block: BlockNo,
        tx: Tx,
        succeeded_once: bool,
    },
    InvalidClearingPrices,
    #[serde(rename_all = "camelCase")]
    MissingPrice {
        token_address: Address,
    },
    InvalidExecutedAmount,
    NonBufferableTokensUsed {
        tokens: BTreeSet<Address>,
    },
    SolverAccountInsufficientBalance {
        #[serde_as(as = "HexOrDecimalU256")]
        required: U256,
    },
    Success {
        transaction: B256,
    },
    Revert {
        transaction: B256,
    },
    DriverError {
        reason: String,
    },
    Cancelled,
    Expired,
    Fail,
    PostprocessingTimedOut,
    DeserializationError {
        reason: String,
    },
}

type BlockNo = u64;

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tx {
    pub from: Address,
    pub to: Address,
    #[serde_as(as = "serde_ext::Hex")]
    pub input: Vec<u8>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    pub access_list: AccessList,
}

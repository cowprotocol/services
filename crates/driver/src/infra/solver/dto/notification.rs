use {
    crate::{
        domain::{
            competition::{auction, solution},
            eth::{self},
        },
        infra::notify,
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
    std::collections::BTreeSet,
    web3::types::AccessList,
};

impl Notification {
    pub fn new(
        auction_id: Option<auction::Id>,
        solution_id: Option<solution::Id>,
        kind: notify::Kind,
    ) -> Self {
        Self {
            auction_id: auction_id.as_ref().map(ToString::to_string),
            solution_id: solution_id.map(SolutionId::from_domain),
            kind: match kind {
                notify::Kind::Timeout => Kind::Timeout,
                notify::Kind::EmptySolution => Kind::EmptySolution,
                notify::Kind::SimulationFailed(block, tx, succeeded_once) => {
                    Kind::SimulationFailed {
                        block: block.0,
                        tx: Tx {
                            from: tx.from.into(),
                            to: tx.to.into(),
                            input: tx.input.into(),
                            value: tx.value.into(),
                            access_list: tx.access_list.into(),
                        },
                        succeeded_once,
                    }
                }
                notify::Kind::ScoringFailed(scoring) => scoring.into(),
                notify::Kind::NonBufferableTokensUsed(tokens) => Kind::NonBufferableTokensUsed {
                    tokens: tokens.into_iter().map(|token| token.0 .0).collect(),
                },
                notify::Kind::SolverAccountInsufficientBalance(required) => {
                    Kind::SolverAccountInsufficientBalance {
                        required: required.0,
                    }
                }
                notify::Kind::DuplicatedSolutionId => Kind::DuplicatedSolutionId,
                notify::Kind::DriverError(reason) => Kind::DriverError { reason },
                notify::Kind::Settled(kind) => match kind {
                    notify::Settlement::Success(hash) => Kind::Success {
                        transaction: hash.0,
                    },
                    notify::Settlement::Revert(hash) => Kind::Revert {
                        transaction: hash.0,
                    },
                    notify::Settlement::SimulationRevert => Kind::Cancelled,
                    notify::Settlement::Fail => Kind::Fail,
                    notify::Settlement::Expired => Kind::Expired,
                },
                notify::Kind::PostprocessingTimedOut => Kind::PostprocessingTimedOut,
                notify::Kind::Banned(reason) => Kind::Banned(match reason {
                    notify::BanReason::UnsettledConsecutiveAuctions => {
                        BanReason::UnsettledConsecutiveAuctions
                    }
                    notify::BanReason::HighSettleFailureRate => BanReason::LowSettlingRate,
                }),
            },
        }
    }
}

impl From<notify::ScoreKind> for Kind {
    fn from(value: notify::ScoreKind) -> Self {
        match value {
            notify::ScoreKind::InvalidClearingPrices => Kind::InvalidClearingPrices,
            notify::ScoreKind::InvalidExecutedAmount => Kind::InvalidExecutedAmount,
            notify::ScoreKind::MissingPrice(token_address) => Kind::MissingPrice {
                token_address: token_address.into(),
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    auction_id: Option<String>,
    solution_id: Option<SolutionId>,
    #[serde(flatten)]
    kind: Kind,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SolutionId {
    Single(u64),
    Merged(Vec<u64>),
}

impl SolutionId {
    pub fn from_domain(id: solution::Id) -> Self {
        match id.solutions().len() {
            1 => SolutionId::Single(*id.solutions().first().unwrap()),
            _ => SolutionId::Merged(id.solutions().to_vec()),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
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
        token_address: eth::H160,
    },
    InvalidExecutedAmount,
    NonBufferableTokensUsed {
        tokens: BTreeSet<eth::H160>,
    },
    SolverAccountInsufficientBalance {
        #[serde_as(as = "serialize::U256")]
        required: eth::U256,
    },
    Success {
        transaction: eth::H256,
    },
    Revert {
        transaction: eth::H256,
    },
    DriverError {
        reason: String,
    },
    Cancelled,
    Expired,
    Fail,
    PostprocessingTimedOut,
    Banned(BanReason),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "reason")]
pub enum BanReason {
    UnsettledConsecutiveAuctions,
    LowSettlingRate,
}

type BlockNo = u64;

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tx {
    pub from: eth::H160,
    pub to: eth::H160,
    #[serde_as(as = "serialize::Hex")]
    pub input: Vec<u8>,
    #[serde_as(as = "serialize::U256")]
    pub value: eth::U256,
    pub access_list: AccessList,
}

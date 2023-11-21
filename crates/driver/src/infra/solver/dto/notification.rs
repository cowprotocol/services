use {
    crate::{
        domain::{
            competition::{auction, solution},
            eth,
        },
        infra::notify,
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
    std::collections::{BTreeSet, HashMap},
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
            solution_id: solution_id.map(|id| id.0),
            kind: match kind {
                notify::Kind::Timeout => Kind::Timeout,
                notify::Kind::EmptySolution => Kind::EmptySolution,
                notify::Kind::SimulationFailed(block, tx) => Kind::SimulationFailed(
                    block.0,
                    Tx {
                        from: tx.from.into(),
                        to: tx.to.into(),
                        input: tx.input.into(),
                        value: tx.value.into(),
                        access_list: tx.access_list.into(),
                    },
                ),
                notify::Kind::ScoringFailed(notify::ScoreKind::ZeroScore) => {
                    Kind::ScoringFailed(ScoreKind::ZeroScore)
                }
                notify::Kind::ScoringFailed(notify::ScoreKind::ScoreHigherThanQuality(
                    score,
                    quality,
                )) => Kind::ScoringFailed(ScoreKind::ScoreHigherThanQuality {
                    score: score.0.get(),
                    quality: quality.0,
                }),
                notify::Kind::ScoringFailed(notify::ScoreKind::SuccessProbabilityOutOfRange(
                    success_probability,
                )) => Kind::ScoringFailed(ScoreKind::SuccessProbabilityOutOfRange {
                    probability: success_probability,
                }),
                notify::Kind::ScoringFailed(notify::ScoreKind::ObjectiveValueNonPositive(
                    quality,
                    gas_cost,
                )) => Kind::ScoringFailed(ScoreKind::ObjectiveValueNonPositive {
                    quality: quality.0,
                    gas_cost: gas_cost.get().0,
                }),
                notify::Kind::NonBufferableTokensUsed(tokens) => Kind::NonBufferableTokensUsed {
                    tokens: tokens.into_iter().map(|token| token.0 .0).collect(),
                },
                notify::Kind::SolverAccountInsufficientBalance(required) => {
                    Kind::SolverAccountInsufficientBalance {
                        required: required.0,
                    }
                }
                notify::Kind::AssetFlow(amounts) => Kind::AssetFlow {
                    amounts: amounts
                        .into_iter()
                        .map(|(token, amount)| (token.0 .0, amount.to_string()))
                        .collect(),
                },
                notify::Kind::DuplicatedSolutionId => Kind::DuplicatedSolutionId,
                notify::Kind::Settled(kind) => Kind::Settled(match kind {
                    notify::Settlement::Success(hash) => Settlement::Success {
                        transaction: hash.0,
                    },
                    notify::Settlement::Revert(hash) => Settlement::Revert {
                        transaction: hash.0,
                    },
                    notify::Settlement::SimulationRevert => Settlement::SimulationRevert,
                    notify::Settlement::Fail => Settlement::Fail,
                }),
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    auction_id: Option<String>,
    solution_id: Option<u64>,
    kind: Kind,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Timeout,
    EmptySolution,
    DuplicatedSolutionId,
    SimulationFailed(BlockNo, Tx),
    ScoringFailed(ScoreKind),
    NonBufferableTokensUsed {
        tokens: BTreeSet<eth::H160>,
    },
    SolverAccountInsufficientBalance {
        #[serde_as(as = "serialize::U256")]
        required: eth::U256,
    },
    AssetFlow {
        amounts: HashMap<eth::H160, String>,
    },
    Settled(Settlement),
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

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreKind {
    ZeroScore,
    ScoreHigherThanQuality {
        #[serde_as(as = "serialize::U256")]
        score: eth::U256,
        #[serde_as(as = "serialize::U256")]
        quality: eth::U256,
    },
    SuccessProbabilityOutOfRange {
        probability: f64,
    },
    #[serde(rename_all = "camelCase")]
    ObjectiveValueNonPositive {
        #[serde_as(as = "serialize::U256")]
        quality: eth::U256,
        #[serde_as(as = "serialize::U256")]
        gas_cost: eth::U256,
    },
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Settlement {
    Success { transaction: eth::H256 },
    Revert { transaction: eth::H256 },
    SimulationRevert,
    Fail,
}

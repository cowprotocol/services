use {
    crate::{
        domain::{
            auction,
            eth,
            notification::{self, SimulationSucceededAtLeastOnce},
        },
        util::serialize,
    },
    ethereum_types::{H160, H256, U256},
    serde::Deserialize,
    serde_with::{serde_as, DisplayFromStr},
    std::collections::BTreeSet,
    web3::types::AccessList,
};

impl Notification {
    /// Converts a data transfer object into its domain object representation.
    pub fn to_domain(&self) -> notification::Notification {
        notification::Notification {
            auction_id: match self.auction_id {
                Some(id) => auction::Id::Solve(id),
                None => auction::Id::Quote,
            },
            solution_id: self.solution_id.map(Into::into),
            kind: match &self.kind {
                Kind::Timeout => notification::Kind::Timeout,
                Kind::EmptySolution => notification::Kind::EmptySolution,
                Kind::SimulationFailed(block, tx, succeeded_at_least_once) => {
                    notification::Kind::SimulationFailed(
                        *block,
                        eth::Tx {
                            from: tx.from.into(),
                            to: tx.to.into(),
                            input: tx.input.clone().into(),
                            value: tx.value.into(),
                            access_list: tx.access_list.clone(),
                        },
                        *succeeded_at_least_once,
                    )
                }
                Kind::ScoringFailed(ScoreKind::ObjectiveValueNonPositive { quality, gas_cost }) => {
                    notification::Kind::ScoringFailed(
                        notification::ScoreKind::ObjectiveValueNonPositive(
                            (*quality).into(),
                            (*gas_cost).into(),
                        ),
                    )
                }
                Kind::ScoringFailed(ScoreKind::ZeroScore) => {
                    notification::Kind::ScoringFailed(notification::ScoreKind::ZeroScore)
                }
                Kind::ScoringFailed(ScoreKind::ScoreHigherThanQuality { score, quality }) => {
                    notification::Kind::ScoringFailed(
                        notification::ScoreKind::ScoreHigherThanQuality(
                            (*score).into(),
                            (*quality).into(),
                        ),
                    )
                }
                Kind::ScoringFailed(ScoreKind::SuccessProbabilityOutOfRange { probability }) => {
                    notification::Kind::ScoringFailed(
                        notification::ScoreKind::SuccessProbabilityOutOfRange(
                            (*probability).into(),
                        ),
                    )
                }
                Kind::NonBufferableTokensUsed { tokens } => {
                    notification::Kind::NonBufferableTokensUsed(
                        tokens
                            .clone()
                            .into_iter()
                            .map(|token| token.into())
                            .collect(),
                    )
                }
                Kind::SolverAccountInsufficientBalance { required } => {
                    notification::Kind::SolverAccountInsufficientBalance(eth::Ether(*required))
                }
                Kind::DuplicatedSolutionId => notification::Kind::DuplicatedSolutionId,
                Kind::Settled(kind) => notification::Kind::Settled(match kind {
                    Settlement::Success { transaction } => {
                        notification::Settlement::Success(*transaction)
                    }
                    Settlement::Revert { transaction } => {
                        notification::Settlement::Revert(*transaction)
                    }
                    Settlement::SimulationRevert => notification::Settlement::SimulationRevert,
                    Settlement::Fail => notification::Settlement::Fail,
                }),
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Notification {
    #[serde_as(as = "Option<DisplayFromStr>")]
    auction_id: Option<i64>,
    solution_id: Option<u64>,
    kind: Kind,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Timeout,
    EmptySolution,
    DuplicatedSolutionId,
    SimulationFailed(BlockNo, Tx, SimulationSucceededAtLeastOnce),
    ScoringFailed(ScoreKind),
    NonBufferableTokensUsed {
        tokens: BTreeSet<H160>,
    },
    SolverAccountInsufficientBalance {
        #[serde_as(as = "serialize::U256")]
        required: U256,
    },
    Settled(Settlement),
}

type BlockNo = u64;

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tx {
    from: H160,
    to: H160,
    #[serde_as(as = "serialize::Hex")]
    input: Vec<u8>,
    #[serde_as(as = "serialize::U256")]
    value: U256,
    access_list: AccessList,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreKind {
    ZeroScore,
    ScoreHigherThanQuality {
        #[serde_as(as = "serialize::U256")]
        score: U256,
        #[serde_as(as = "serialize::U256")]
        quality: U256,
    },
    SuccessProbabilityOutOfRange {
        probability: f64,
    },
    #[serde(rename_all = "camelCase")]
    ObjectiveValueNonPositive {
        #[serde_as(as = "serialize::U256")]
        quality: U256,
        #[serde_as(as = "serialize::U256")]
        gas_cost: U256,
    },
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Settlement {
    Success { transaction: H256 },
    Revert { transaction: H256 },
    SimulationRevert,
    Fail,
}

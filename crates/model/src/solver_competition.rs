use {
    crate::order::OrderUid,
    alloy_primitives::{Address, U256},
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionAuction {
    pub orders: Vec<OrderUid>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<Address, U256>,
}

#[serde_as]
#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SolverSettlement {
    pub solver: String,
    #[serde(default)]
    pub solver_address: Address,
    #[serde(flatten)]
    pub score: Option<Score>,
    #[serde(default)]
    pub ranking: usize,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub clearing_prices: BTreeMap<Address, U256>,
    pub orders: Vec<Order>,
    #[serde(default)]
    pub is_winner: bool,
    #[serde(default)]
    pub filtered_out: bool,
}

#[serde_as]
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Score {
    /// The score is provided by the solver.
    #[serde(rename = "score")]
    Solver(#[serde_as(as = "HexOrDecimalU256")] U256),
    /// The score is calculated by the protocol (and equal to the objective
    /// function).
    #[serde(rename = "scoreProtocol")]
    Protocol(#[serde_as(as = "HexOrDecimalU256")] U256),
    /// The score is calculated by the protocol and success_probability provided
    /// by solver is taken into account
    #[serde(rename = "scoreProtocolWithSolverRisk")]
    ProtocolWithSolverRisk(#[serde_as(as = "HexOrDecimalU256")] U256),
    /// The score is calculated by the protocol, by applying a discount to the
    /// `Self::Protocol` value.
    /// [DEPRECATED] Kept to not brake the solver competition API.
    #[serde(rename = "scoreDiscounted")]
    Discounted(#[serde_as(as = "HexOrDecimalU256")] U256),
}

impl Default for Score {
    fn default() -> Self {
        Self::Protocol(Default::default())
    }
}

impl Score {
    pub fn score(&self) -> U256 {
        match self {
            Self::Solver(score) => *score,
            Self::Protocol(score) => *score,
            Self::ProtocolWithSolverRisk(score) => *score,
            Self::Discounted(score) => *score,
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum Order {
    #[serde(rename_all = "camelCase")]
    Colocated {
        id: OrderUid,
        /// The effective amount that left the user's wallet including all fees.
        #[serde_as(as = "HexOrDecimalU256")]
        sell_amount: U256,
        /// The effective amount the user received after all fees.
        #[serde_as(as = "HexOrDecimalU256")]
        buy_amount: U256,
    },
    #[serde(rename_all = "camelCase")]
    Legacy {
        id: OrderUid,
        #[serde_as(as = "HexOrDecimalU256")]
        executed_amount: U256,
    },
}

use {
    super::serialize,
    number::serialization::HexOrDecimalU256,
    serde::Deserialize,
    serde_with::{serde_as, DisplayFromStr},
    std::collections::BTreeSet,
    utoipa::{
        openapi::{ObjectBuilder, RefOr, Schema, SchemaType},
        ToSchema,
    },
    web3::types::{AccessList, H160, H256, U256},
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub auction_id: Option<i64>,
    pub solution_id: Option<SolutionId>,
    #[serde(flatten)]
    pub kind: Kind,
}

// serde(flatten) has a conflict with the current API schema
impl ToSchema<'static> for Notification {
    fn schema() -> (&'static str, RefOr<Schema>) {
        let auction_id = ObjectBuilder::new()
            .description(Some(
                "The auction ID of the auction that the solution was providedfor.",
            ))
            .schema_type(SchemaType::String);
        let solution_id = ObjectBuilder::new()
            .description(Some(
                "The solution ID within the auction for which the notification applies",
            ))
            .schema_type(SchemaType::Number);
        let kind = ObjectBuilder::new()
            .schema_type(SchemaType::String)
            .enum_values(Some([
                "timeout",
                "emptySolution",
                "duplicatedSolutionId",
                "simulationFailed",
                "invalidClearingPrices",
                "missingPrice",
                "invalidExecutedAmount",
                "nonBufferableTokensUsed",
                "solverAccountInsufficientBalance",
                "success",
                "revert",
                "driverError",
                "cancelled",
                "fail",
                "postprocessingTimedOut",
            ]));
        let notification = ObjectBuilder::new()
            .description(Some(
                "A notification that informs the solver how its solution performed in the \
                 auction. Depending on the notification type additional meta data may be attached \
                 but this is not guaranteed to be stable.",
            ))
            .schema_type(SchemaType::Object)
            .property("auctionId", auction_id)
            .property("solutionId", solution_id)
            .property("kind", kind)
            .build();

        ("Notification", Schema::Object(notification).into())
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SolutionId {
    Single(u64),
    Merged(Vec<u64>),
}

#[serde_as]
#[derive(Debug, Deserialize)]
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
        token_address: H160,
    },
    InvalidExecutedAmount,
    NonBufferableTokensUsed {
        tokens: BTreeSet<H160>,
    },
    SolverAccountInsufficientBalance {
        #[serde_as(as = "HexOrDecimalU256")]
        required: U256,
    },
    Success {
        transaction: H256,
    },
    Revert {
        transaction: H256,
    },
    DriverError {
        reason: String,
    },
    Cancelled,
    Fail,
    PostprocessingTimedOut,
}

type BlockNo = u64;

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tx {
    pub from: H160,
    pub to: H160,
    #[serde_as(as = "serialize::Hex")]
    pub input: Vec<u8>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    pub access_list: AccessList,
}

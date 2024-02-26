use {
    serde::{Deserialize, Serialize},
    utoipa::ToSchema,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    #[schema(example = "DuplicatedOrder")]
    error_type: String,
    #[schema(example = "string")]
    description: String,
}

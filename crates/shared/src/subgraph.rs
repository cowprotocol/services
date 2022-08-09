//! A module implementing a client for querying subgraphs.

use anyhow::{bail, Result};
use lazy_static::lazy_static;
use reqwest::{Client, IntoUrl, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;

const QUERY_PAGE_SIZE: usize = 1000;

/// A general client for querying subgraphs.
pub struct SubgraphClient {
    client: Client,
    subgraph_url: Url,
}

lazy_static! {
    pub static ref DEFAULT_GRAPH_API_BASE_URL: Url =
        Url::parse("https://api.thegraph.com/subgraphs/name/")
            .expect("invalid default Graph API base URL");
}

pub trait ContainsId {
    fn get_id(&self) -> String;
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Data<T> {
    #[serde(alias = "pools", alias = "ticks")]
    pub inner: Vec<T>,
}

impl SubgraphClient {
    /// Creates a new subgraph client from the specified organization and name.
    pub fn new(org: impl AsRef<str>, name: impl AsRef<str>, client: Client) -> Result<Self> {
        Self::with_base_url(DEFAULT_GRAPH_API_BASE_URL.clone(), org, name, client)
    }

    /// Creates a new subgraph client with the specified base URL.
    pub fn with_base_url(
        base_url: impl IntoUrl,
        org: impl AsRef<str>,
        name: impl AsRef<str>,
        client: Client,
    ) -> Result<Self> {
        let subgraph_url = base_url
            .into_url()?
            .join(&format!("{}/", org.as_ref()))?
            .join(name.as_ref())?;
        Ok(Self {
            client,
            subgraph_url,
        })
    }

    /// Performs the specified GraphQL query on the current subgraph.
    pub async fn query<T>(&self, query: &str, variables: Option<Map<String, Value>>) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.client
            .post(self.subgraph_url.clone())
            .json(&Query { query, variables })
            .send()
            .await?
            .json::<QueryResponse<T>>()
            .await?
            .into_result()
    }

    /// Performs the specified GraphQL query on the current subgraph.
    /// This function should be called for queries that return very long(paginated) result.
    pub async fn paginated_query<T>(&self, block_number: u64, query: &str) -> Result<Vec<T>>
    where
        T: ContainsId + DeserializeOwned,
    {
        let mut result = Vec::new();
        let mut last_id = String::default();

        // We do paging by last ID instead of using `skip`. This is the
        // suggested approach to paging best performance:
        // <https://thegraph.com/docs/en/developer/graphql-api/#pagination>
        loop {
            let page = self
                .query::<Data<T>>(
                    query,
                    Some(json_map! {
                        "block" => block_number,
                        "pageSize" => QUERY_PAGE_SIZE,
                        "lastId" => json!(last_id),
                    }),
                )
                .await?
                .inner;
            let no_more_pages = page.len() != QUERY_PAGE_SIZE;
            if let Some(last_pool) = page.last() {
                last_id = last_pool.get_id();
            }

            result.extend(page);

            if no_more_pages {
                break;
            }
        }

        Ok(result)
    }
}

/// A GraphQL query.
#[derive(Serialize)]
struct Query<'a> {
    query: &'a str,
    variables: Option<Map<String, Value>>,
}

/// A GraphQL query response.
///
/// This type gets converted into a Rust `Result` type, while handling invalid
/// responses (with missing data and errors).
#[derive(Debug, Deserialize)]
struct QueryResponse<T> {
    #[serde(default = "empty_data")]
    data: Option<T>,
    #[serde(default)]
    errors: Option<Vec<QueryError>>,
}

impl<T> QueryResponse<T> {
    fn into_result(self) -> Result<T> {
        match self {
            Self {
                data: Some(data),
                errors: None,
            } => Ok(data),
            Self {
                errors: Some(errors),
                data: None,
            } if !errors.is_empty() => {
                // Make sure to log additional errors if there are more than
                // one, and just bubble up the first error.
                for error in &errors[1..] {
                    tracing::warn!("additional GraphQL error: {}", error.message);
                }
                bail!("{}", errors[0])
            }
            _ => bail!("invalid GraphQL response"),
        }
    }
}

#[derive(Debug, Deserialize, Error)]
#[error("{}", .message)]
struct QueryError {
    message: String,
}

/// Function to work around the fact that `#[serde(default)]` on an `Option<T>`
/// requires `T: Default`.
fn empty_data<T>() -> Option<T> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn serialize_query() {
        assert_eq!(
            serde_json::to_value(&Query {
                query: r#"foo {
                }"#,
                variables: Some(json_map! {
                    "foo" => "bar",
                    "baz" => 42,
                    "thing" => false,
                }),
            })
            .unwrap(),
            json!({
                "query": "foo {\n                }",
                "variables": {
                    "foo": "bar",
                    "baz": 42,
                    "thing": false,
                },
            }),
        );
    }

    fn response_from_json<T>(value: Value) -> Result<T>
    where
        T: DeserializeOwned,
    {
        serde_json::from_value::<QueryResponse<T>>(value)
            .unwrap()
            .into_result()
    }

    #[test]
    fn deserialize_successful_response() {
        assert!(response_from_json::<bool>(json!({ "data": true })).unwrap());
    }

    #[test]
    fn deserialize_error_response() {
        assert_eq!(
            response_from_json::<bool>(json!({
                "data": null,
                "errors": [{"message": "foo"}],
            }))
            .unwrap_err()
            .to_string(),
            "foo",
        );
        assert_eq!(
            response_from_json::<bool>(json!({
                "errors": [{"message": "bar"}],
            }))
            .unwrap_err()
            .to_string(),
            "bar",
        );
    }

    #[test]
    fn deserialize_multi_error_response() {
        assert_eq!(
            response_from_json::<bool>(json!({
                "data": null,
                "errors": [
                    {"message": "foo"},
                    {"message": "bar"},
                ],
            }))
            .unwrap_err()
            .to_string(),
            "foo",
        );
    }

    #[test]
    fn deserialize_invalid_response() {
        assert!(response_from_json::<bool>(json!({
            "data": null,
            "errors": null,
        }))
        .is_err());
        assert!(response_from_json::<bool>(json!({
            "data": null,
            "errors": [],
        }))
        .is_err());
        assert!(response_from_json::<bool>(json!({
            "data": true,
            "errors": [],
        }))
        .is_err());
        assert!(response_from_json::<bool>(json!({
            "data": true,
            "errors": [{"message":"bad"}],
        }))
        .is_err());
    }
}

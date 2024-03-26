//! A module implementing a client for querying subgraphs.

use {
    anyhow::{bail, Result},
    reqwest::{Client, Url},
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    serde_json::{json, Map, Value},
    thiserror::Error,
};

pub const QUERY_PAGE_SIZE: usize = 1000;
const MAX_NUMBER_OF_RETRIES: usize = 10;

/// A general client for querying subgraphs.
pub struct SubgraphClient {
    client: Client,
    subgraph_url: Url,
}

pub trait ContainsId {
    fn get_id(&self) -> String;
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Data<T> {
    #[serde(alias = "pools", alias = "ticks")]
    pub inner: Vec<T>,
}

impl SubgraphClient {
    /// Creates a new subgraph client from the specified organization and name.
    pub fn new(subgraph_url: Url, client: Client) -> Result<Self> {
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
        // for long lasting queries subgraph call might randomly fail
        // introduced retry mechanism that should efficiently help since failures are
        // quick and we need 1 or 2 retries to succeed.
        for _ in 0..MAX_NUMBER_OF_RETRIES {
            match self
                .client
                .post(self.subgraph_url.clone())
                .json(&Query {
                    query,
                    variables: variables.clone(),
                })
                .send()
                .await?
                .json::<QueryResponse<T>>()
                .await?
                .into_result()
            {
                Ok(result) => return Ok(result),
                Err(err) => tracing::warn!("failed to query subgraph: {}", err),
            }
        }
        Err(anyhow::anyhow!("failed to execute query on subgraph"))
    }

    /// Performs the specified GraphQL query on the current subgraph.
    /// This function should be called for queries that return very
    /// long(paginated) result.
    pub async fn paginated_query<T>(
        &self,
        query: &str,
        mut variables: Map<String, Value>,
    ) -> Result<Vec<T>>
    where
        T: ContainsId + DeserializeOwned,
    {
        let mut result = Vec::new();

        // We do paging by last ID instead of using `skip`. This is the
        // suggested approach to paging best performance:
        // <https://thegraph.com/docs/en/developer/graphql-api/#pagination>
        variables.extend(json_map! {
            "pageSize" => QUERY_PAGE_SIZE,
            "lastId" => json!(String::default()),
        });
        loop {
            let page = self
                .query::<Data<T>>(query, Some(variables.clone()))
                .await?
                .inner;
            let no_more_pages = page.len() != QUERY_PAGE_SIZE;
            if let Some(last_elem) = page.last() {
                variables.insert("lastId".to_string(), json!(last_elem.get_id()));
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
    use {
        super::*,
        serde_json::{json, Value},
    };

    #[test]
    fn serialize_query() {
        assert_eq!(
            serde_json::to_value(Query {
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

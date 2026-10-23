//! The app data endpoints: the documents the orders' 32-byte `app_data`
//! hashes are the keccak256 of. A client registers a document ahead of or
//! alongside its order and reads one back by its hash.

use {
    crate::infra::{
        api::{State, error, extract},
        db,
    },
    app_data::{AppDataDocument, AppDataHash, hash_full_app_data},
    axum::{Json, http::StatusCode},
};

/// Handle `GET /api/v1/app_data/{hash}`.
pub async fn get_app_data(
    state: axum::extract::State<State>,
    extract::PathAppDataHash(hash): extract::PathAppDataHash,
) -> Result<Json<AppDataDocument>, error::Reply> {
    let full_app_data = db::find_app_data(state.pool(), hash.0)
        .await
        .map_err(|err| {
            tracing::error!(?err, "app data lookup failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        })?
        .ok_or_else(|| {
            error::reply(StatusCode::NOT_FOUND, "NotFound", "full app data not found")
        })?;
    Ok(Json(AppDataDocument { full_app_data }))
}

/// Handle `PUT /api/v1/app_data`: register a document and answer its hash.
pub async fn put_app_data(
    state: axum::extract::State<State>,
    extract::Json(document): extract::Json<AppDataDocument>,
) -> Result<(StatusCode, Json<AppDataHash>), error::Reply> {
    register(&state, None, &document.full_app_data).await
}

/// Handle `PUT /api/v1/app_data/{hash}`: register a document under the hash
/// the client computed for it.
pub async fn put_app_data_by_hash(
    state: axum::extract::State<State>,
    extract::PathAppDataHash(hash): extract::PathAppDataHash,
    extract::Json(document): extract::Json<AppDataDocument>,
) -> Result<(StatusCode, Json<AppDataHash>), error::Reply> {
    register(&state, Some(hash), &document.full_app_data).await
}

/// Store a document under its hash: `201` when new, `200` when the same
/// document is stored already. The error names are the EVM orderbook's.
async fn register(
    state: &State,
    expected: Option<AppDataHash>,
    document: &str,
) -> Result<(StatusCode, Json<AppDataHash>), error::Reply> {
    let hash = validate(document, state.app_data_size_limit()).map_err(|description| {
        error::reply(StatusCode::BAD_REQUEST, "AppDataInvalid", description)
    })?;
    if let Some(expected) = expected
        && expected != hash
    {
        return Err(error::reply(
            StatusCode::BAD_REQUEST,
            "AppDataHashMismatch",
            format!("computed appDataHash {hash:?} doesn't match expected {expected:?}"),
        ));
    }
    let existing = db::insert_app_data(state.pool(), hash.0, document)
        .await
        .map_err(|err| {
            tracing::error!(?err, "app data insert failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        })?;
    match existing {
        None => Ok((StatusCode::CREATED, Json(hash))),
        Some(existing) if existing == document => Ok((StatusCode::OK, Json(hash))),
        Some(existing) => Err(error::reply(
            StatusCode::BAD_REQUEST,
            "AppDataMismatch",
            format!("stored appData {existing:?} is different than the specified data"),
        )),
    }
}

/// The hash a valid document is named by. Valid is a JSON object of at most
/// `size_limit` bytes. The protocol `metadata` is not parsed: the EVM schema
/// types its hooks, partner fees, and signer as EVM addresses, and no Solana
/// component reads it.
pub fn validate(document: &str, size_limit: usize) -> Result<AppDataHash, String> {
    if document.len() > size_limit {
        return Err(format!(
            "app data has byte size {} which is larger than limit {size_limit}",
            document.len()
        ));
    }
    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(document)
        .map_err(|err| format!("app data is not a JSON object: {err}"))?;
    Ok(AppDataHash(hash_full_app_data(document.as_bytes())))
}

/// The document a request carries, in the EVM orderbook's forms: `appData`
/// is the document or its hash, and `appDataHash` the hash the client
/// computed for the document. Every hash named must agree, the `order`'s
/// signed one first. `None` without a document: the hash forms name one
/// registered on its own, or none at all.
pub fn document(
    app_data: Option<&str>,
    app_data_hash: Option<AppDataHash>,
    order: Option<AppDataHash>,
    size_limit: usize,
) -> Result<Option<&str>, Error> {
    let mut hashes: Vec<AppDataHash> = order.into_iter().chain(app_data_hash).collect();
    let document = match app_data {
        None => None,
        Some(value) => match value.parse::<AppDataHash>() {
            Ok(hash) => {
                hashes.push(hash);
                None
            }
            Err(_) => Some(value),
        },
    };
    if let Some(document) = document {
        hashes.push(validate(document, size_limit).map_err(Error::Invalid)?);
    }
    if let Some((expected, named)) = hashes.split_first()
        && let Some(found) = named.iter().find(|hash| *hash != expected)
    {
        return Err(Error::HashMismatch {
            expected: *expected,
            found: *found,
        });
    }
    Ok(document)
}

/// Why a request's app data is refused. The reply names are the EVM
/// orderbook's for order placement.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Invalid(String),
    /// A named hash disagrees with the one the request is checked against.
    HashMismatch {
        expected: AppDataHash,
        found: AppDataHash,
    },
}

impl From<Error> for error::Reply {
    fn from(error: Error) -> Self {
        match error {
            Error::Invalid(description) => {
                error::reply(StatusCode::BAD_REQUEST, "InvalidAppData", description)
            }
            Error::HashMismatch { expected, found } => error::reply(
                StatusCode::BAD_REQUEST,
                "AppDataHashMismatch",
                format!("app data hash {found:?} doesn't match {expected:?}"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: usize = 8192;

    fn hash(document: &str) -> AppDataHash {
        AppDataHash(hash_full_app_data(document.as_bytes()))
    }

    /// A JSON object within the limit hashes as the EVM orderbook hashes it,
    /// anything else is refused.
    #[test]
    fn validates_a_json_object_within_the_limit() {
        let empty: AppDataHash =
            "0xb48d38f93eaa084033fc5970bf96e559c33c4cdc07d889ab00b4d63f9590739d"
                .parse()
                .unwrap();
        assert_eq!(validate("{}", LIMIT), Ok(empty));
        assert!(validate("{}", 1).is_err());
        assert!(validate("[]", LIMIT).is_err());
        assert!(validate("\"{}\"", LIMIT).is_err());
        assert!(validate("not json", LIMIT).is_err());
    }

    /// Every hash a request names must agree with the order's, and the
    /// document with every hash. Only a document is returned: the hash forms
    /// name one registered on its own.
    #[test]
    fn hashes_must_agree() {
        let empty = hash("{}");
        let other = AppDataHash([0x11; 32]);
        let empty_hex = format!("{empty:?}");
        let other_hex = format!("{other:?}");
        for (app_data, app_data_hash, order, expected) in [
            (None, None, Some(empty), Ok(None)),
            (None, Some(empty), Some(empty), Ok(None)),
            (Some(empty_hex.as_str()), None, Some(empty), Ok(None)),
            (Some(empty_hex.as_str()), Some(empty), None, Ok(None)),
            (Some("{}"), None, Some(empty), Ok(Some("{}"))),
            (Some("{}"), Some(empty), Some(empty), Ok(Some("{}"))),
            (Some("{}"), Some(empty), None, Ok(Some("{}"))),
            (Some("{}"), None, None, Ok(Some("{}"))),
            // The order's hash is the reference the rest is checked against.
            (
                None,
                Some(other),
                Some(empty),
                Err(Error::HashMismatch {
                    expected: empty,
                    found: other,
                }),
            ),
            (
                Some(other_hex.as_str()),
                None,
                Some(empty),
                Err(Error::HashMismatch {
                    expected: empty,
                    found: other,
                }),
            ),
            (
                Some("{}"),
                None,
                Some(other),
                Err(Error::HashMismatch {
                    expected: other,
                    found: empty,
                }),
            ),
            (
                Some("{}"),
                Some(other),
                Some(empty),
                Err(Error::HashMismatch {
                    expected: empty,
                    found: other,
                }),
            ),
            // Without an order, the client's own hash is the reference.
            (
                Some("{}"),
                Some(other),
                None,
                Err(Error::HashMismatch {
                    expected: other,
                    found: empty,
                }),
            ),
        ] {
            assert_eq!(
                document(app_data, app_data_hash, order, LIMIT),
                expected,
                "{app_data:?} {app_data_hash:?} {order:?}"
            );
        }
        assert!(matches!(
            document(Some("[]"), None, Some(empty), LIMIT),
            Err(Error::Invalid(_))
        ));
    }
}

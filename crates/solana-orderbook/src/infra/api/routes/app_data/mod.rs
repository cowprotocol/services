//! The app data endpoints: the documents the orders' 32-byte `app_data`
//! hashes are the keccak256 of. A client registers a document ahead of or
//! alongside its order and reads one back by its hash.

use {
    crate::infra::{
        api::{State, error, extract},
        db,
    },
    app_data::{AppDataDocument, AppDataHash, EMPTY, OrderCreationAppData, hash_full_app_data},
    axum::{Json, http::StatusCode},
    sqlx::PgPool,
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
        error::reply(
            StatusCode::BAD_REQUEST,
            "AppDataInvalid",
            format!("appData is invalid: {description}"),
        )
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

/// The document stored under `hash`. The zero hash names the empty document
/// by protocol convention.
pub async fn find(pool: &PgPool, hash: AppDataHash) -> anyhow::Result<Option<String>> {
    if hash.is_zero() {
        return Ok(Some(EMPTY.to_owned()));
    }
    db::find_app_data(pool, hash.0).await
}

/// The document an `app_data` form names, checked the EVM orderbook's way: a
/// document must hash to the `appDataHash` named with it, a hash alone must
/// have its document `stored` already, and either must be the `intent`'s
/// signed hash.
pub fn resolve(
    app_data: &OrderCreationAppData,
    stored: Option<&str>,
    intent: Option<AppDataHash>,
    size_limit: usize,
) -> Result<String, Error> {
    let hash = match app_data {
        OrderCreationAppData::Both { full, expected } => {
            let hash = validate(full, size_limit).map_err(Error::Invalid)?;
            if hash != *expected {
                return Err(Error::HashMismatch {
                    provided: *expected,
                    actual: hash,
                });
            }
            hash
        }
        OrderCreationAppData::Full { full } => {
            validate(full, size_limit).map_err(Error::Invalid)?
        }
        OrderCreationAppData::Hash { hash } => *hash,
    };
    if let Some(intent) = intent
        && hash != intent
    {
        return Err(Error::HashMismatch {
            provided: intent,
            actual: hash,
        });
    }
    match app_data {
        OrderCreationAppData::Both { full, .. } | OrderCreationAppData::Full { full } => {
            Ok(full.clone())
        }
        OrderCreationAppData::Hash { hash } => {
            let Some(stored) = stored else {
                return Err(Error::Invalid(format!(
                    "Unknown pre-image for app data hash {hash:?}"
                )));
            };
            validate(stored, size_limit).map_err(Error::Invalid)?;
            Ok(stored.to_owned())
        }
    }
}

/// Why a request's app data is refused. The reply names and wording are the
/// EVM orderbook's for order placement.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Invalid(String),
    /// A hash the request named or signed, and the one its document has.
    HashMismatch {
        provided: AppDataHash,
        actual: AppDataHash,
    },
}

impl From<Error> for error::Reply {
    fn from(error: Error) -> Self {
        match error {
            Error::Invalid(description) => {
                error::reply(StatusCode::BAD_REQUEST, "InvalidAppData", description)
            }
            Error::HashMismatch { provided, actual } => error::reply(
                StatusCode::BAD_REQUEST,
                "AppDataHashMismatch",
                format!(
                    "calculated app data hash {actual:?} doesn't match order app data field \
                     {provided:?}"
                ),
            ),
        }
    }
}

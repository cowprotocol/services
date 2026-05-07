//! Shared helpers for simulator integration tests.
//!
//! Cargo treats files under `tests/` as independent crates, so any helper
//! used by more than one test must live under `tests/common/` (the `mod`
//! suffix keeps cargo from compiling this as its own test binary).

/// Returns `app_data` minified with object keys sorted alphabetically.
///
/// The sort comes from `serde_json::Value::Object`'s `BTreeMap` backing,
/// which applies whenever the `preserve_order` feature is not enabled (it
/// isn't in this workspace). Production app-data payloads happen to already
/// be alphabetically keyed at every level, so the output matches them byte
/// for byte and the resulting `AppDataHash` lines up with the on-chain hash.
pub fn canonicalise_app_data(app_data: &str) -> String {
    let value: serde_json::Value =
        serde_json::from_str(app_data).expect("app_data must be valid JSON");
    serde_json::to_string(&value).expect("re-serialising must succeed")
}

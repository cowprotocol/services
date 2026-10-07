// This is an empty stub for `sqlx-mysql` to replace the registry crate via
// `[patch.crates-io]`. The real `sqlx-mysql` pulls in the vulnerable `rsa`
// crate (RUSTSEC-2023-0071), but we never enable the sqlx `mysql` feature,
// so the stub is never actually compiled against — it only exists to prevent
// cargo from resolving `rsa` into `Cargo.lock`.

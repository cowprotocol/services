/// The current time.
#[cfg(not(test))]
pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

/// During tests, the time is fixed.
#[cfg(test)]
pub fn now() -> chrono::DateTime<chrono::Utc> {
    use std::sync::OnceLock;
    static TIME: OnceLock<chrono::DateTime<chrono::Utc>> = OnceLock::new();
    TIME.get_or_init(chrono::Utc::now).to_owned()
}

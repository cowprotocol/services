/// The current time.
#[cfg(not(test))]
pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

/// During tests, the time is fixed.
#[cfg(test)]
pub fn now() -> chrono::DateTime<chrono::Utc> {
    use std::sync::LazyLock;
    static TIME: LazyLock<chrono::DateTime<chrono::Utc>> = LazyLock::new(chrono::Utc::now);
    *TIME
}

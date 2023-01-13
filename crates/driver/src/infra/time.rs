/// The current time.
#[derive(Debug, Clone, Copy)]
pub enum Now {
    /// Return the time according to this machine's clock.
    Real,
    /// Always return the same time. Used for testing.
    Fake(chrono::DateTime<chrono::Utc>),
}

impl Now {
    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            Self::Real => chrono::Utc::now(),
            Self::Fake(time) => time.to_owned(),
        }
    }
}

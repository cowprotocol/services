use {crate::infra, thiserror::Error};

/// A datetime representing a deadline until something needs to be done.
#[derive(Clone, Copy, Debug, Default)]
pub struct Deadline(chrono::DateTime<chrono::Utc>);

impl Deadline {
    pub fn remaining(self) -> Result<chrono::Duration, DeadlineExceeded> {
        let deadline = self.0 - infra::time::now();
        if deadline < chrono::Duration::zero() {
            Err(DeadlineExceeded)
        } else {
            Ok(deadline)
        }
    }

    pub fn reduce(self, duration: chrono::Duration) -> Self {
        Self(self.0 - duration)
    }
}

impl From<chrono::DateTime<chrono::Utc>> for Deadline {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self(value)
    }
}

#[derive(Debug, Error)]
#[error("the deadline has been exceeded")]
pub struct DeadlineExceeded;

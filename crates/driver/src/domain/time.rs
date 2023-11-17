use {
    crate::{
        infra::{self},
        util,
    },
    thiserror::Error,
};

/// A datetime representing a deadline until something needs to be done.
#[derive(Clone, Copy, Debug, Default)]
pub struct Deadline(chrono::DateTime<chrono::Utc>);

impl Deadline {
    /// Remaining time until the deadline is reached.
    pub fn remaining(self) -> Result<chrono::Duration, DeadlineExceeded> {
        let deadline = self.0 - infra::time::now();
        if deadline < chrono::Duration::zero() {
            Err(DeadlineExceeded)
        } else {
            Ok(deadline)
        }
    }

    /// Returns a new deadline that is reduced by the given percentage.
    pub fn reduce(self, percentage: util::Percent) -> Self {
        let duration = self.0 - infra::time::now();
        let leftover = 100.0 - percentage.get();
        Self(infra::time::now() + duration * leftover.round() as i32 / 100)
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

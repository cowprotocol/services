use {crate::infra, thiserror::Error};

/// A datetime representing a deadline until something needs to be done.
#[derive(Clone, Copy, Debug, Default)]
pub struct Deadline(chrono::DateTime<chrono::Utc>);

impl Deadline {
    pub fn remaining(self) -> Result<Remaining, DeadlineExceeded> {
        self.try_into()
    }

    pub fn reduce(self, duration: chrono::Duration) -> Self {
        Self(self.0 - duration)
    }
}

impl TryFrom<Deadline> for Remaining {
    type Error = DeadlineExceeded;

    fn try_from(value: Deadline) -> Result<Remaining, DeadlineExceeded> {
        let deadline = value.0 - infra::time::now();
        if deadline < chrono::Duration::zero() {
            Err(DeadlineExceeded)
        } else {
            Ok(Remaining(deadline))
        }
    }
}

impl From<chrono::DateTime<chrono::Utc>> for Deadline {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self(value)
    }
}

/// Remaining duration until the deadline is reached.
#[derive(Clone, Copy, Debug)]
pub struct Remaining(chrono::Duration);

impl Remaining {
    pub fn duration(&self) -> chrono::Duration {
        self.0
    }
}

#[derive(Debug, Error)]
#[error("the deadline has been exceeded")]
pub struct DeadlineExceeded;

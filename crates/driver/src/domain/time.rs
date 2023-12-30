use {
    crate::infra::{
        observe,
        solver::Timeouts,
        {self},
    },
    thiserror::Error,
};

/// Deadlines for different parts of the driver execution.
/// The driver is expected to return the solution to the autopilot before the
/// driver deadline.
/// The solvers are expected to return the solution to the driver before the
/// solvers deadline.
#[derive(Copy, Clone, Debug, Default)]
pub struct Deadline {
    driver: chrono::DateTime<chrono::Utc>,
    solvers: chrono::DateTime<chrono::Utc>,
}

impl Deadline {
    pub fn new(deadline: chrono::DateTime<chrono::Utc>, timeouts: Timeouts) -> Self {
        let deadline = deadline - timeouts.http_delay;
        let deadline = Self {
            driver: deadline,
            solvers: {
                let now = infra::time::now();
                let duration = deadline - now;
                now + duration * (timeouts.solving_share_of_deadline.get() * 100.0).round() as i32
                    / 100
            },
        };
        observe::deadline(&deadline, &timeouts);
        deadline
    }

    /// Remaining time until the deadline for driver to return solution to
    /// autopilot is reached.
    pub fn driver(self) -> chrono::DateTime<chrono::Utc> {
        self.driver
    }

    /// Remaining time until the deadline for solvers to return solution to
    /// driver is reached.
    pub fn solvers(self) -> chrono::DateTime<chrono::Utc> {
        self.solvers
    }
}

pub trait Remaining {
    fn remaining(self) -> Result<std::time::Duration, DeadlineExceeded>;
}
impl Remaining for chrono::DateTime<chrono::Utc> {
    fn remaining(self) -> Result<std::time::Duration, DeadlineExceeded> {
        let deadline = self - infra::time::now();
        if deadline <= chrono::Duration::zero() {
            Err(DeadlineExceeded)
        } else {
            Ok(deadline.to_std().expect("not negative"))
        }
    }
}

#[derive(Debug, Error)]
#[error("the deadline has been exceeded")]
pub struct DeadlineExceeded;

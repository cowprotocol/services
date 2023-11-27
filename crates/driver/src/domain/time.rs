use {
    crate::infra::{
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
        Self {
            driver: deadline,
            solvers: {
                let now = infra::time::now();
                let duration = deadline - now;
                now + duration * (timeouts.solving_share_of_deadline.get() * 100.0).round() as i32
                    / 100
            },
        }
    }

    /// Remaining time until the deadline for driver to return solution to
    /// autopilot is reached.
    pub fn driver(self) -> Result<std::time::Duration, DeadlineExceeded> {
        Self::remaining(self.driver)
    }

    /// Remaining time until the deadline for solvers to return solution to
    /// driver is reached.
    pub fn solvers(self) -> Result<std::time::Duration, DeadlineExceeded> {
        Self::remaining(self.solvers)
    }

    fn remaining(
        deadline: chrono::DateTime<chrono::Utc>,
    ) -> Result<std::time::Duration, DeadlineExceeded> {
        let deadline = deadline - infra::time::now();
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

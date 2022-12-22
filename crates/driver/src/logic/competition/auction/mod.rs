pub use order::Order;
use {crate::logic::eth, std::collections::HashMap, thiserror::Error};

pub mod order;

/// An auction is a set of orders that can be solved. The solvers calculate
/// [`super::solution::Solution`]s by picking subsets of these orders and
/// solving them.
#[derive(Debug)]
pub struct Auction {
    pub id: Id,
    pub block: eth::BlockNo,
    pub orders: Vec<Order>,
    pub deadline: Deadline,
    pub prices: HashMap<eth::Token, eth::Ether>,
}

/// Each auction has a deadline, limiting the maximum time that each solver may
/// allocate to solving the auction.
#[derive(Debug, Clone)]
pub struct Deadline(chrono::DateTime<chrono::Utc>);

impl From<chrono::DateTime<chrono::Utc>> for Deadline {
    fn from(inner: chrono::DateTime<chrono::Utc>) -> Self {
        Self(inner)
    }
}

impl Deadline {
    /// The time limit passed to the solver. The solvers is given a time limit
    /// that's slightly less than the actual deadline. The reason for this
    /// is to allow the solver to use the full deadline to search for the
    /// most optimal solution, but still ensure there is time left for the
    /// driver to forward the results back to the protocol or do some other
    /// necessary work.
    ///
    /// This method returns [`std::time::Duration`] rather than
    /// [`chrono::Duration`] because [`std::time::Duration`] is guaranteed
    /// to be nonnegative, while [`chrono::Duration`] can be negative as well.
    pub fn solver_time_limit(&self) -> Result<std::time::Duration, DeadlineExceeded> {
        let timeout = self.0 - chrono::Utc::now() - Self::time_buffer();
        if timeout <= chrono::Duration::zero() {
            Err(DeadlineExceeded)
        } else {
            Ok(timeout
                .to_std()
                .expect("already checked that the duration is positive"))
        }
    }

    fn time_buffer() -> chrono::Duration {
        chrono::Duration::seconds(1)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Id(pub u64);

impl From<u64> for Id {
    fn from(inner: u64) -> Self {
        Self(inner)
    }
}

#[derive(Debug, Error)]
#[error("the deadline has been exceeded")]
pub struct DeadlineExceeded;

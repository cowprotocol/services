//! Legacy HTTP solver adapter implementation.
//!
//! In order to faciliate the transition from the legacy HTTP solver API to the
//! new HTTP API, we provide a solver "wrapper" that just marshals the API
//! request types to and from the legacy format.

use crate::domain::eth;

pub struct Legacy {
    pub chain: eth::ChainId,
    pub weth: eth::WethAddress,
}

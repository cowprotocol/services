//! Legacy HTTP solver adapter implementation.
//!
//! In order to faciliate the transition from the legacy HTTP solver API to the
//! new HTTP API, we provide a solver "wrapper" that just marshals the API
//! request types to and from the legacy format.

// TODO I guess we can simply move the actual code here?
pub use crate::boundary::legacy::Legacy;

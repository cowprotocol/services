//! Module containing all the necessary pieces to trace logs across
//! multiple services by passing open telemetry information via HTTP headers.

pub mod request_id;
pub mod trace_id_format;
pub mod axum;
pub mod headers;

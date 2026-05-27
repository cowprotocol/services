//! Wire-format types for events published on the protocol event bus.
//!
//! Every event implements [`Event`], which pairs the Rust type with the NATS
//! subject suffix the bus uses to publish it. Keeping the DTOs here — separate
//! from the producers — lets external consumers depend on a small,
//! dependency-light crate, and lets the bundled CLI emit JSON schemas for the
//! full set of events.

pub mod price_estimate;

pub use price_estimate::PriceEstimateEvent;
use {schemars::JsonSchema, serde::Serialize};

/// An event that can be published on the event bus.
pub trait Event: Serialize + JsonSchema {
    /// NATS subject suffix this event is published under (without the
    /// `event.<chain_id>.` prefix added by the publisher).
    const SUBJECT: &'static str;
}

/// One entry per known event type. Drives the CLI and any other place that
/// needs to enumerate the full set of events (e.g. tests that pin the wire
/// format).
pub fn schemas() -> Vec<(&'static str, schemars::Schema)> {
    vec![(
        PriceEstimateEvent::SUBJECT,
        schemars::schema_for!(PriceEstimateEvent),
    )]
}

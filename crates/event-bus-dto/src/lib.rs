//! Wire-format types for events published on the protocol event bus.
//!
//! Every event implements [`Event`], which pairs the Rust type with the NATS
//! subject suffix the bus uses to publish it. Each event is wrapped in an
//! [`Envelope`] that carries the wire-format version, a timestamp and the
//! request id that correlates events belonging to the same inbound request.
//!
//! Keeping the DTOs here — separate from the producers — lets external
//! consumers depend on a small, dependency-light crate, and lets the bundled
//! `event-bus-schemas` CLI emit JSON schemas for the full set of events.

pub mod envelope;
pub mod price_estimate;
pub mod query;
pub mod quote_computed;
pub mod quote_requested;
pub mod winning_price_estimate;

pub use {
    envelope::{ENVELOPE_VERSION, Envelope},
    price_estimate::PriceEstimateEvent,
    query::{OrderKind, QueryFields},
    quote_computed::QuoteComputedEvent,
    quote_requested::QuoteRequestedEvent,
    winning_price_estimate::WinningPriceEstimateEvent,
};
use {schemars::JsonSchema, serde::Serialize};

/// An event that can be published on the event bus.
pub trait Event: Serialize + JsonSchema {
    /// NATS subject suffix this event is published under (without the
    /// `event.<chain_id>.` prefix added by the publisher).
    const SUBJECT: &'static str;
}

/// Pairs each event's NATS subject with the JSON schema of its full
/// [`Envelope`]-wrapped wire format. Listing an event type here is all that's
/// needed to include it in [`schemas`].
macro_rules! event_schemas {
    ($($event:ty),+ $(,)?) => {
        vec![$((
            <$event as Event>::SUBJECT,
            schemars::schema_for!(Envelope<$event>),
        )),+]
    };
}

/// One entry per known event type, mapping the NATS subject to the JSON schema
/// of the full [`Envelope`]-wrapped wire format consumers receive. Drives the
/// CLI and any other place that needs to enumerate the full set of events
/// (e.g. tests that pin the wire format).
pub fn schemas() -> Vec<(&'static str, schemars::Schema)> {
    event_schemas![
        PriceEstimateEvent,
        QuoteRequestedEvent,
        QuoteComputedEvent,
        WinningPriceEstimateEvent
    ]
}

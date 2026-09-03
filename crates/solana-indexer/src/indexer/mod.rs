//! Consumer components of the Solana settlement indexer.
//!
//! The four components and their roles:
//!
//! - [`Ingester`]: subscribes to the Yellowstone gRPC stream and drains it as
//!   fast as updates arrive, forwarding them to the decoder. It does no
//!   decoding itself, so the socket never backs up behind slow processing. It
//!   is also the single writer of the "latest chain slot" counter that the
//!   other components use to know how far the chain has advanced.
//!
//! - [`Decoder`]: receives the raw stream updates, picks out transactions
//!   belonging to the settlement and SolFlow programs, and persists the
//!   resulting typed events to the store.
//!
//! Rows are written at the `confirmed` commitment level. The stream's
//! finalized slot statuses advance `solana.indexer_state.finalized_slot`, and
//! a row counts as final once its slot is at or below that watermark.

pub mod decoder;
pub mod ingester;

#[expect(unused_imports)]
pub(crate) use {decoder::Decoder, ingester::Ingester};

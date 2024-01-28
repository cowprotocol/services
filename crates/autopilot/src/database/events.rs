use {
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, events::EventIndex, OrderUid},
    ethcontract::EventMetadata,
    std::convert::TryInto,
};

pub fn meta_to_event_index(meta: &EventMetadata) -> EventIndex {
    EventIndex {
        block_number: meta.block_number as i64,
        log_index: meta.log_index as i64,
    }
}

pub fn bytes_to_order_uid(bytes: &[u8]) -> Result<OrderUid> {
    bytes
        .try_into()
        .context("order_uid has wrong number of bytes")
        .map(ByteArray)
}

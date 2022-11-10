use std::{
    convert::TryInto,
    time::{Duration, SystemTime},
};

/// The current time in the same unit as `valid_to` for orders in the smart contract.
pub fn now_in_epoch_seconds() -> u32 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("now earlier than epoch")
        .as_secs()
        .try_into()
        .expect("epoch seconds larger than max u32")
}

/// Adds a `std::time::Duration` to a `u32` timestamp. This function uses
/// saturating semantics and can't panic.
pub fn timestamp_after_duration(timestamp: u32, duration: Duration) -> u32 {
    timestamp.saturating_add(duration.as_secs().try_into().unwrap_or(u32::MAX))
}

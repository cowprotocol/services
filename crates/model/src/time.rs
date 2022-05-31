use std::{convert::TryInto, time::SystemTime};

/// The current time in the same unit as `valid_to` for orders in the smart contract.
pub fn now_in_epoch_seconds() -> u32 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("now earlier than epoch")
        .as_secs()
        .try_into()
        .expect("epoch seconds larger than max u32")
}

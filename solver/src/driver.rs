use crate::ethereum::SettlementContract;
use std::{sync::Arc, time::Duration};

pub struct Driver {
    contract: Arc<dyn SettlementContract>,
    max_order_age: Duration,
}

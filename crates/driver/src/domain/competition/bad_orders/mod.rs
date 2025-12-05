use {
    crate::domain::competition::{order, Auction},
    std::fmt,
};

pub mod metrics;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Quality {
    /// Order is likely to produce working solutions when included.
    Supported,
    /// Order will likely produce failing solutions when included.
    /// This can have many reasons:
    /// * order-specific issues (bad pre/post interactions, signature problems)
    /// * insufficient balance or approval
    /// * order targeting problematic tokens
    /// * malicious or buggy order parameters
    Unsupported,
    /// The detection strategy does not have enough data to make an informed
    /// decision.
    Unknown,
}

#[derive(Default)]
pub struct Detector {
    metrics: Option<metrics::Detector>,
}

impl Detector {
    /// Creates a new detector without any detection mechanisms enabled.
    pub fn new() -> Self {
        Self {
            metrics: None,
        }
    }

    /// Enables detection of unsupported orders based on settlement simulation
    /// failure heuristics.
    pub fn with_metrics_detector(&mut self, detector: metrics::Detector) -> &mut Self {
        self.metrics = Some(detector);
        self
    }

    /// Removes all unsupported orders from the auction.
    pub fn filter_unsupported_orders_in_auction(&self, mut auction: Auction) -> Auction {
        let now = std::time::Instant::now();

        // reuse the original allocation
        let all_orders = std::mem::take(&mut auction.orders);
        let mut removed_uids = Vec::new();

        let supported_orders: Vec<_> = all_orders
            .into_iter()
            .filter_map(|order| {
                let quality = self.get_order_quality(&order.uid, now);
                match quality {
                    Quality::Supported | Quality::Unknown => Some(order),
                    Quality::Unsupported => {
                        removed_uids.push(order.uid);
                        None
                    }
                }
            })
            .collect();

        auction.orders = supported_orders;
        if !removed_uids.is_empty() {
            tracing::debug!(orders = ?removed_uids, "ignored orders flagged as unsupported");
        }

        auction
    }

    /// Updates the order quality metric for successful settlements.
    pub fn encoding_succeeded(&self, order_uids: &[order::Uid]) {
        if let Some(metrics) = &self.metrics {
            metrics.update_orders(order_uids, false);
        }
    }

    /// Updates the order quality metric for failed settlements.
    pub fn encoding_failed(&self, order_uids: &[order::Uid]) {
        if let Some(metrics) = &self.metrics {
            metrics.update_orders(order_uids, true);
        }
    }

    fn get_order_quality(&self, uid: &order::Uid, now: std::time::Instant) -> Quality {
        if let Some(Quality::Unsupported) = self.metrics.as_ref().map(|m| m.get_quality(uid, now))
        {
            return Quality::Unsupported;
        }

        Quality::Unknown
    }
}

impl fmt::Debug for Detector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Detector").finish()
    }
}

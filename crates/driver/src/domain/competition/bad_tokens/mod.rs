use {
    crate::domain::{competition::Auction, eth},
    futures::StreamExt,
    std::{collections::HashMap, fmt, time::Instant},
};

pub mod cache;
pub mod metrics;
pub mod simulation;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Quality {
    /// Solver is likely to produce working solutions when computing
    /// routes for this token.
    Supported,
    /// Solver will likely produce failing solutions when computing
    /// routes for this token. This can have many reasons:
    /// * fees on transfer
    /// * token enforces max transfer amount
    /// * trader is deny listed
    /// * bugs in the solidity compiler make it incompatible with the settlement
    ///   contract - see <https://github.com/cowprotocol/services/pull/781>
    /// * probably tons of other reasons
    Unsupported,
}

#[derive(Default)]
pub struct Detector {
    /// manually configured list of supported and unsupported tokens. Only
    /// tokens that get detected incorrectly by the automatic detectors get
    /// listed here and therefore have a higher precedence.
    hardcoded: HashMap<eth::TokenAddress, Quality>,
    simulation_detector: Option<simulation::Detector>,
    metrics: Option<metrics::Detector>,
}

impl Detector {
    /// Hardcodes tokens as (un)supported based on the provided config. This has
    /// the highest priority when looking up a token's quality.
    pub fn new(config: HashMap<eth::TokenAddress, Quality>) -> Self {
        Self {
            hardcoded: config,
            ..Default::default()
        }
    }

    /// Enables detection of unsupported tokens via simulation based detection
    /// methods.
    pub fn with_simulation_detector(&mut self, detector: simulation::Detector) -> &mut Self {
        self.simulation_detector = Some(detector);
        self
    }

    /// Enables detection of unsupported tokens based on heuristics.
    pub fn enable_heuristic_detector(&mut self) -> &mut Self {
        self.metrics = Some(metrics::Detector::default());
        self
    }

    /// Removes all unsupported orders from the auction.
    pub async fn filter_unsupported_orders_in_auction(&self, mut auction: Auction) -> Auction {
        let now = Instant::now();

        let filtered_orders = futures::stream::iter(auction.orders.into_iter())
            .filter_map(move |order| async move {
                let sell = self.get_token_quality(order.sell.token, now);
                let buy = self.get_token_quality(order.sell.token, now);
                match (sell, buy) {
                    // both tokens supported => keep order
                    (Some(Quality::Supported), Some(Quality::Supported)) => Some(order),
                    // at least 1 token unsupported => drop order
                    (Some(Quality::Unsupported), _) | (_, Some(Quality::Unsupported)) => None,
                    // sell token quality is unknown => keep order if token is supported
                    (None, _) => {
                        let Some(detector) = &self.simulation_detector else {
                            // we can't determine quality => assume order is good
                            return Some(order);
                        };
                        let quality = detector.determine_sell_token_quality(&order, now).await;
                        (quality == Some(Quality::Supported)).then_some(order)
                    }
                    // buy token quality is unknown => keep order (because we can't
                    // determine quality and assume it's good)
                    (_, None) => Some(order),
                }
            })
            .collect::<Vec<_>>()
            .await;
        auction.orders = filtered_orders;

        if let Some(detector) = &self.simulation_detector {
            detector.evict_outdated_entries();
        }

        auction
    }

    /// Updates the tokens quality metric for successful operation.
    pub fn encoding_succeeded(&self, token_pairs: &[(eth::TokenAddress, eth::TokenAddress)]) {
        if let Some(metrics) = &self.metrics {
            metrics.update_tokens(token_pairs, false);
        }
    }

    /// Updates the tokens quality metric for failures.
    pub fn encoding_failed(&self, token_pairs: &[(eth::TokenAddress, eth::TokenAddress)]) {
        if let Some(metrics) = &self.metrics {
            metrics.update_tokens(token_pairs, true);
        }
    }

    fn get_token_quality(&self, token: eth::TokenAddress, now: Instant) -> Option<Quality> {
        if let Some(quality) = self.hardcoded.get(&token) {
            return Some(*quality);
        }

        if let Some(detector) = &self.simulation_detector {
            if let Some(quality) = detector.get_quality(token, now) {
                return Some(quality);
            }
        }

        if let Some(metrics) = &self.metrics {
            return metrics.get_quality(&token);
        }

        None
    }
}

impl fmt::Debug for Detector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Detector")
            .field("hardcoded", &self.hardcoded)
            .finish()
    }
}

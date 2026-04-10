//! This module implements logic to detect risky orders that
//! a solver is not able to support. The module supports
//! flagging individual tokens that are not supported outright.
//! A bad token could for example be one that forbids trading
//! with AMMs, only allows 1 transfer per transaction/block, or
//! was simply built with a buggy compiler which makes it incompatible
//! with the settlement contract (see <https://github.com/cowprotocol/services/pull/781>).
//!
//! Additionally, there are some heuristics to detect when an
//! order itself is somehow broken or causes issues and slipped through
//! other detection mechanisms. One big error case is orders adjusting
//! debt positions in lending protocols. While pre-checks might correctly
//! detect that the EIP 1271 signature is valid the transfer of the token
//! would fail because the user's debt position is not collateralized enough.
//! In other words the bad order detection is a last fail-safe in case
//! we were not able to predict issues with orders and pre-emptively
//! filter them out of the auction.

use {
    crate::domain::competition::{Order, order::Uid},
    eth_domain_types as eth,
    futures::{StreamExt, stream::FuturesUnordered},
    std::{
        collections::{HashMap, HashSet},
        fmt,
        time::Instant,
    },
};

pub mod bad_orders;
pub mod bad_tokens;

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
    /// The detection strategy does not have enough data to make an informed
    /// decision.
    Unknown,
}

#[derive(Default)]
pub struct Detector {
    /// manually configured list of supported and unsupported tokens. Only
    /// tokens that get detected incorrectly by the automatic detectors get
    /// listed here and therefore have a higher precedence.
    hardcoded: HashMap<eth::TokenAddress, Quality>,
    simulation_detector: Option<bad_tokens::simulation::Detector>,
    metrics: Option<bad_orders::metrics::Detector>,
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
    pub fn with_simulation_detector(
        &mut self,
        detector: bad_tokens::simulation::Detector,
    ) -> &mut Self {
        self.simulation_detector = Some(detector);
        self
    }

    /// Enables detection of unsupported tokens based on heuristics.
    pub fn with_metrics_detector(&mut self, detector: bad_orders::metrics::Detector) -> &mut Self {
        self.metrics = Some(detector);
        self
    }

    /// Filters unsupported orders out of the auction.
    pub async fn filter_unsupported_orders_in_auction(
        &self,
        mut auction: crate::domain::competition::Auction,
    ) -> crate::domain::competition::Auction {
        let removed_uids = self.unsupported_order_uids(&auction.orders).await;
        if !removed_uids.is_empty() {
            auction
                .orders
                .retain(|order| !removed_uids.contains(&order.uid));
        }
        auction
    }

    /// Returns the UIDs of orders this solver cannot support.
    pub async fn unsupported_order_uids(&self, orders: &[Order]) -> HashSet<Uid> {
        let now = Instant::now();
        let mut token_quality_checks = FuturesUnordered::new();
        let mut removed_uids = HashSet::new();

        for order in orders {
            if self
                .metrics
                .as_ref()
                .map(|metrics| metrics.get_quality(&order.uid, now))
                .is_some_and(|q| q == Quality::Unsupported)
            {
                removed_uids.insert(order.uid);
                continue;
            }

            let sell = self.get_token_quality(order.sell.token, now);
            let buy = self.get_token_quality(order.buy.token, now);

            match (sell, buy) {
                // sell token quality is unknown => keep order if token is supported
                (Quality::Supported, Quality::Supported) => {}
                // at least 1 token unsupported => drop order
                (Quality::Unsupported, _) | (_, Quality::Unsupported) => {
                    removed_uids.insert(order.uid);
                }
                // sell token quality is unknown => keep order if token is supported
                (Quality::Unknown, _) => {
                    // we can't determine quality => assume order is good
                    let Some(detector) = &self.simulation_detector else {
                        continue;
                    };

                    let order = order.clone();
                    let check_tokens_fut = async move {
                        let quality = detector.determine_sell_token_quality(&order, now).await;
                        (order.uid, quality)
                    };
                    token_quality_checks.push(check_tokens_fut);
                }
                // buy token quality is unknown => keep order (because we can't determine quality
                // and assume it's good)
                (_, Quality::Unknown) => {}
            }
        }

        while let Some((uid, quality)) = token_quality_checks.next().await {
            if quality != Quality::Supported {
                removed_uids.insert(uid);
            }
        }

        if !removed_uids.is_empty() {
            tracing::debug!(orders = ?removed_uids, "ignored orders with unsupported tokens");
        }

        if let Some(detector) = &self.simulation_detector {
            detector.evict_outdated_entries();
        }

        removed_uids
    }

    /// Updates the tokens quality metric for successful operation.
    pub fn encoding_succeeded(&self, orders: &[Uid]) {
        if let Some(metrics) = &self.metrics {
            metrics.update_orders(orders, false);
        }
    }

    /// Updates the tokens quality metric for failures.
    pub fn encoding_failed(&self, orders: &[Uid]) {
        if let Some(metrics) = &self.metrics {
            metrics.update_orders(orders, true);
        }
    }

    fn get_token_quality(&self, token: eth::TokenAddress, now: Instant) -> Quality {
        match self.hardcoded.get(&token) {
            None | Some(Quality::Unknown) => (),
            Some(quality) => return *quality,
        }

        self.simulation_detector
            .as_ref()
            .map(|d| d.get_quality(&token, now))
            .unwrap_or(Quality::Unknown)
    }
}

impl fmt::Debug for Detector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Detector")
            .field("hardcoded", &self.hardcoded)
            .finish()
    }
}

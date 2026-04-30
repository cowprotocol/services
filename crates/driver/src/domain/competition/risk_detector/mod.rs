//! This module implements logic to detect risky orders that
//! a solver is not able to support. The module supports
//! flagging individual tokens that are not supported outright.
//! A bad token could for example be one that forbids trading
//! with AMMs, only allows 1 transfer per transaction/block, or
//! was simply built with a buggy compiler which makes it incompatible
//! with the settlement contract (see <https://github.com/cowprotocol/services/pull/781>).
//!
//! Additionally there are some heuristics to detect when an
//! order itself is somehow broken or causes issues and slipped through
//! other detection mechanisms. One big error case is orders adjusting
//! debt postions in lending protocols. While pre-checks might correctly
//! detect that the EIP 1271 signature is valid the transfer of the token
//! would fail because the user's debt position is not collateralized enough.
//! In other words the bad order detection is a last fail safe in case
//! we were not able to predict issues with orders and pre-emptively
//! filter them out of the auction.

use {
    crate::domain::competition::{Auction, Order, order::Uid},
    eth_domain_types as eth,
    futures::{StreamExt, stream::FuturesUnordered},
    std::{
        collections::{HashMap, HashSet},
        fmt,
        ops::Deref,
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

#[async_trait::async_trait]
pub trait SellQualityDetector: Send + Sync {
    async fn determine_sell_token_quality(&self, order: &Order, now: Instant) -> Quality;
    fn get_quality(&self, token: &eth::TokenAddress, now: Instant) -> Quality;
    fn evict_outdated_entries(&self);
}

#[async_trait::async_trait]
impl SellQualityDetector for bad_tokens::simulation::Detector {
    async fn determine_sell_token_quality(&self, order: &Order, now: Instant) -> Quality {
        self.determine_sell_token_quality(order, now).await
    }

    fn get_quality(&self, token: &eth::TokenAddress, now: Instant) -> Quality {
        Deref::deref(self).get_quality(token, now)
    }

    fn evict_outdated_entries(&self) {
        Deref::deref(self).evict_outdated_entries()
    }
}

#[derive(Default)]
pub struct Detector {
    /// manually configured list of supported and unsupported tokens. Only
    /// tokens that get detected incorrectly by the automatic detectors get
    /// listed here and therefore have a higher precedence.
    hardcoded: HashMap<eth::TokenAddress, Quality>,
    simulation_detector: Option<Box<dyn SellQualityDetector>>,
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
        detector: impl SellQualityDetector + 'static,
    ) -> &mut Self {
        self.simulation_detector = Some(Box::new(detector));
        self
    }

    /// Enables detection of unsupported tokens based on heuristics.
    pub fn with_metrics_detector(&mut self, detector: bad_orders::metrics::Detector) -> &mut Self {
        self.metrics = Some(detector);
        self
    }

    /// Removes all unsupported orders from the auction.
    pub async fn filter_unsupported_orders_in_auction(
        &self,
        auction: &mut Auction,
        flashloans: bool,
    ) {
        // Filter out orders that require flashloans if flashloans is disabled.
        if !flashloans {
            auction.orders.retain(|o| o.app_data.flashloan().is_none());
        }
        let unsupported_uids = self.unsupported_order_uids(auction.orders()).await;
        // Filter out unsupported orders
        if !unsupported_uids.is_empty() {
            auction
                .orders
                .retain(|order| !unsupported_uids.contains(&order.uid));
        }
    }

    /// Returns a set of orders uids for orders that are found to be
    /// unsupported.
    pub async fn unsupported_order_uids(&self, orders: &[Order]) -> HashSet<Uid> {
        let now = Instant::now();

        let mut token_quality_checks = FuturesUnordered::new();
        let mut removed_uids = Vec::new();

        orders.iter().for_each(|order| {
            if self
                .metrics
                .as_ref()
                .map(|metrics| metrics.get_quality(&order.uid, now))
                .is_some_and(|q| q == Quality::Unsupported)
            {
                removed_uids.push(order.uid);
                return;
            }

            let sell = self.get_token_quality(order.sell.token, now);
            let buy = self.get_token_quality(order.buy.token, now);

            match (sell, buy) {
                // at least 1 token unsupported => drop order
                (Quality::Unsupported, _) | (_, Quality::Unsupported) => {
                    removed_uids.push(order.uid);
                }

                // sell token quality is unknown => keep order if token is supported
                (Quality::Unknown, _) => {
                    let Some(detector) = &self.simulation_detector else {
                        // we can't determine quality => assume order is good
                        return;
                    };

                    let check_tokens_fut = async move {
                        let quality = detector.determine_sell_token_quality(order, now).await;
                        (order.uid, quality)
                    };
                    token_quality_checks.push(check_tokens_fut);
                }

                // both tokens supported => keep order
                (Quality::Supported, Quality::Supported) => {}

                // buy token quality is unknown => keep order (because we can't
                // determine quality and assume it's good)
                (_, Quality::Unknown) => {}
            }
        });

        while let Some((uid, quality)) = token_quality_checks.next().await {
            if quality != Quality::Supported {
                removed_uids.push(uid);
            }
        }

        if !removed_uids.is_empty() {
            tracing::debug!(
                orders = ?removed_uids,
                "ignored orders with unsupported tokens"
            );
        }

        if let Some(detector) = &self.simulation_detector {
            detector.evict_outdated_entries();
        }

        removed_uids.into_iter().collect()
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            domain::competition::{
                Order,
                order::{
                    BuyTokenBalance,
                    Kind,
                    Partial,
                    SellTokenBalance,
                    Side,
                    Signature,
                    Uid,
                    signature,
                },
            },
            infra::solver,
            util,
        },
        eth_domain_types::TokenAmount,
        std::time::Duration,
    };

    // Helper to create a mock order purely for test
    fn order(
        uid: Uid,
        signer: eth::Address,
        sell_token: eth::TokenAddress,
        buy_token: eth::TokenAddress,
        valid_to: u32,
    ) -> Order {
        Order {
            uid,
            receiver: Some(signer),
            created: util::Timestamp(0),
            valid_to: util::Timestamp(valid_to),
            buy: eth::Asset {
                token: buy_token,
                amount: TokenAmount::from(1),
            },
            sell: eth::Asset {
                token: sell_token,
                amount: TokenAmount::from(1),
            },
            side: Side::Sell,
            kind: Kind::Limit,
            app_data: Default::default(),
            partial: Partial::No,
            pre_interactions: vec![],
            post_interactions: vec![],
            sell_token_balance: SellTokenBalance::Erc20,
            buy_token_balance: BuyTokenBalance::Erc20,
            signature: Signature {
                scheme: signature::Scheme::PreSign,
                data: Default::default(),
                signer,
            },
            protocol_fees: Default::default(),
            quote: Default::default(),
        }
    }

    // Helper to create a mock UID purely for test
    fn uid(n: u8, signer: eth::Address, valid_to: u32) -> Uid {
        let order_hash = eth::B256::from([n; 32]);
        Uid::from_parts(order_hash, signer, valid_to)
    }

    struct TestSellQualityDetector {
        sell_detector_unsupported_uid: Uid,
        #[allow(dead_code)]
        sell_detector_supported_uid: Uid,
    }

    #[async_trait::async_trait]
    impl SellQualityDetector for TestSellQualityDetector {
        async fn determine_sell_token_quality(&self, order: &Order, _: Instant) -> Quality {
            if order.uid == self.sell_detector_unsupported_uid {
                Quality::Unsupported
            } else {
                Quality::Supported
            }
        }

        fn get_quality(&self, _: &eth::TokenAddress, _: Instant) -> Quality {
            Quality::Unknown
        }

        fn evict_outdated_entries(&self) {}
    }

    // Helper to create a mock sell quality detector purely for test
    fn sell_quality_detector(supported: Uid, unsupported: Uid) -> TestSellQualityDetector {
        TestSellQualityDetector {
            sell_detector_unsupported_uid: unsupported,
            sell_detector_supported_uid: supported,
        }
    }

    #[tokio::test]
    async fn unsupported_order_uids_empty_returns_empty() {
        let detector = Detector::new(Default::default());
        let removed = detector.unsupported_order_uids(&[]).await;
        assert!(removed.is_empty());
    }

    #[tokio::test]
    async fn all_supported_orders_are_kept() {
        let signer = eth::Address::from_slice(&[1; 20]);
        let sell_token = eth::Address::from_slice(&[2; 20]).into();
        let buy_token = eth::Address::from_slice(&[3; 20]).into();

        let detector = Detector::new(Default::default());

        let removed = detector
            .unsupported_order_uids(&[order(
                uid(1, signer, u32::MAX),
                signer,
                sell_token,
                buy_token,
                u32::MAX,
            )])
            .await;

        assert!(removed.is_empty());
    }

    #[tokio::test]
    async fn unsupported_order_uids_returns_only_unsupported_orders() {
        fn addr(n: u8) -> eth::Address {
            eth::Address::from_slice(&[n; 20])
        }

        let valid_to = u32::MAX;

        let orders = vec![
            order(
                uid(1, addr(6), valid_to),
                addr(6),
                addr(1).into(),
                addr(2).into(),
                valid_to,
            ), // metrics bad
            order(
                uid(2, addr(7), valid_to),
                addr(7),
                addr(3).into(),
                addr(2).into(),
                valid_to,
            ), // token bad
            order(
                uid(3, addr(8), valid_to),
                addr(8),
                addr(1).into(),
                addr(2).into(),
                valid_to,
            ), // token supported
            order(
                uid(4, addr(9), valid_to),
                addr(9),
                addr(4).into(),
                addr(2).into(),
                valid_to,
            ), // unknown sell
            order(
                uid(5, addr(10), valid_to),
                addr(10),
                addr(1).into(),
                addr(5).into(),
                valid_to,
            ), // unknown buy
            order(
                uid(6, addr(11), valid_to),
                addr(11),
                addr(6).into(),
                addr(2).into(),
                valid_to,
            ), // unknown sell unsupported
            order(
                uid(7, addr(12), valid_to),
                addr(12),
                addr(7).into(),
                addr(2).into(),
                valid_to,
            ), // unknown sell supported
        ];

        let metrics_uid = orders[0].uid;
        let token_uid = orders[1].uid;
        let sell_detector_unsupported_uid = orders[5].uid;
        let sell_detector_supported_uid = orders[6].uid;

        let metrics_detector = bad_orders::metrics::Detector::new(
            0.5,
            2,
            false,
            Duration::from_secs(60),
            Duration::from_secs(60),
            Duration::from_secs(60),
            solver::Name("test_solver".into()),
        );

        let mut detector_config = HashMap::new();
        detector_config.insert(addr(3).into(), Quality::Unsupported);

        let mut detector = Detector::new(detector_config);
        detector.with_metrics_detector(metrics_detector);

        // Simulate repeated metrics failure for order with metrics_uid
        detector.encoding_failed(&[metrics_uid]);
        detector.encoding_failed(&[metrics_uid]);
        detector.encoding_failed(&[metrics_uid]);

        detector.with_simulation_detector(sell_quality_detector(
            sell_detector_supported_uid,
            sell_detector_unsupported_uid,
        ));

        let removed = detector.unsupported_order_uids(&orders).await;

        assert_eq!(
            removed,
            HashSet::from([metrics_uid, token_uid, sell_detector_unsupported_uid])
        ); // all unsupported removed
        assert!(!removed.contains(&orders[2].uid)); // supported token kept
        assert!(!removed.contains(&orders[4].uid)); // unknown buy kept
        assert!(!removed.contains(&sell_detector_supported_uid)); // supported unknown sell kept
    }
}

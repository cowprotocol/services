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

    /// Returns the UIDs of orders this solver cannot support.
    pub async fn unsupported_order_uids(&self, orders: &[Order]) -> HashSet<Uid> {
        let now = Instant::now();
        let mut token_quality_checks = FuturesUnordered::new();
        let mut unsupported_order_uids = HashSet::new();

        for order in orders {
            if self
                .metrics
                .as_ref()
                .map(|metrics| metrics.get_quality(&order.uid, now))
                .is_some_and(|q| q == Quality::Unsupported)
            {
                unsupported_order_uids.insert(order.uid);
                continue;
            }
            let sell = self.get_token_quality(order.sell.token, now);
            let buy = self.get_token_quality(order.buy.token, now);
            match (sell, buy) {
                // sell token quality is unknown => keep order if token is supported
                (Quality::Supported, Quality::Supported) => {}
                // at least 1 token unsupported => drop order
                (Quality::Unsupported, _) | (_, Quality::Unsupported) => {
                    unsupported_order_uids.insert(order.uid);
                }
                // sell token quality is unknown => keep order if token is supported
                (Quality::Unknown, _) => {
                    let Some(detector) = &self.simulation_detector else {
                        // we can't determine quality => assume order is good
                        continue;
                    };
                    let check_tokens_fut = async move {
                        let quality = detector.determine_sell_token_quality(order, now).await;
                        (order.uid, quality)
                    };
                    token_quality_checks.push(check_tokens_fut);
                }
                // buy token quality is unknown => keep order (because we can't
                // determine quality and assume it's good)
                (_, Quality::Unknown) => {}
            }
        }

        while let Some((uid, quality)) = token_quality_checks.next().await {
            if quality != Quality::Supported {
                unsupported_order_uids.insert(uid);
            }
        }

        if !unsupported_order_uids.is_empty() {
            tracing::debug!(orders = ?unsupported_order_uids, "ignored orders with unsupported tokens");
        }

        if let Some(detector) = &self.simulation_detector {
            detector.evict_outdated_entries();
        }

        unsupported_order_uids
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
            domain::competition::order::{
                self,
                BuyTokenBalance,
                Kind,
                Partial,
                SellTokenBalance,
                Side,
                Signature,
                signature::Scheme,
            },
            util,
        },
        alloy::primitives::Bytes,
        eth_domain_types::{Address, Asset, TokenAddress, TokenAmount, U256},
    };

    fn token(byte: u8) -> TokenAddress {
        TokenAddress::from(Address::repeat_byte(byte))
    }

    fn order(uid_byte: u8, sell: TokenAddress, buy: TokenAddress) -> Order {
        Order {
            uid: [uid_byte; order::UID_LEN].into(),
            receiver: None,
            created: util::Timestamp(0),
            valid_to: util::Timestamp(u32::MAX),
            buy: Asset {
                token: buy,
                amount: TokenAmount(U256::from(1u64)),
            },
            sell: Asset {
                token: sell,
                amount: TokenAmount(U256::from(1u64)),
            },
            side: Side::Sell,
            kind: Kind::Market,
            app_data: Default::default(),
            partial: Partial::No,
            pre_interactions: vec![],
            post_interactions: vec![],
            sell_token_balance: SellTokenBalance::Erc20,
            buy_token_balance: BuyTokenBalance::Erc20,
            signature: Signature {
                scheme: Scheme::Eip1271,
                data: Bytes::default(),
                signer: Default::default(),
            },
            protocol_fees: vec![],
            quote: None,
        }
    }

    #[tokio::test]
    async fn empty_orders_returns_empty_set() {
        let detector = Detector::default();
        assert!(detector.unsupported_order_uids(&[]).await.is_empty());
    }

    #[tokio::test]
    async fn both_tokens_supported_keeps_order() {
        let a = token(1);
        let b = token(2);
        let detector = Detector::new(
            [(a, Quality::Supported), (b, Quality::Supported)]
                .into_iter()
                .collect(),
        );
        assert!(
            detector
                .unsupported_order_uids(&[order(1, a, b)])
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn unsupported_sell_token_flags_order() {
        let bad = token(1);
        let good = token(2);
        let detector = Detector::new(
            [(bad, Quality::Unsupported), (good, Quality::Supported)]
                .into_iter()
                .collect(),
        );
        let o = order(1, bad, good);
        let set = detector
            .unsupported_order_uids(std::slice::from_ref(&o))
            .await;
        assert_eq!(set.len(), 1);
        assert!(set.contains(&o.uid));
    }

    #[tokio::test]
    async fn unsupported_buy_token_flags_order() {
        let good = token(1);
        let bad = token(2);
        let detector = Detector::new(
            [(good, Quality::Supported), (bad, Quality::Unsupported)]
                .into_iter()
                .collect(),
        );
        let o = order(1, good, bad);
        let set = detector
            .unsupported_order_uids(std::slice::from_ref(&o))
            .await;
        assert_eq!(set.len(), 1);
        assert!(set.contains(&o.uid));
    }

    #[tokio::test]
    async fn unknown_tokens_without_simulation_are_kept() {
        // No hardcoded entries and no simulation detector → unknown quality,
        // order is assumed supported.
        let detector = Detector::default();
        let o = order(1, token(1), token(2));
        assert!(detector.unsupported_order_uids(&[o]).await.is_empty());
    }

    #[tokio::test]
    async fn mixed_batch_only_flags_offending_orders() {
        let good = token(1);
        let bad = token(9);
        let detector = Detector::new(
            [(good, Quality::Supported), (bad, Quality::Unsupported)]
                .into_iter()
                .collect(),
        );
        let clean = order(1, good, good);
        let bad_sell = order(2, bad, good);
        let bad_buy = order(3, good, bad);
        let set = detector
            .unsupported_order_uids(&[clean.clone(), bad_sell.clone(), bad_buy.clone()])
            .await;
        assert_eq!(set.len(), 2);
        assert!(!set.contains(&clean.uid));
        assert!(set.contains(&bad_sell.uid));
        assert!(set.contains(&bad_buy.uid));
    }
}

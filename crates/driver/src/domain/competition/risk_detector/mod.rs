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
    crate::domain::competition::{
        Order,
        order::Uid,
        risk_detector::bad_tokens::simulation::SellQualityDetector,
    },
    eth_domain_types as eth,
    futures::{StreamExt, stream::FuturesUnordered},
    std::{
        collections::{HashMap, HashSet},
        fmt,
        time::Instant,
    },
    tracing::instrument,
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

    #[instrument(skip_all)]
    /// Performs flashloan filtering and removes all unsupported orders from the
    /// auction.
    pub async fn without_unsupported_orders(
        &self,
        orders: &mut Vec<Order>,
        flashloans_enabled: bool,
    ) {
        if !flashloans_enabled {
            orders.retain(|o| o.app_data.flashloan().is_none());
        }
        self.filter_unsupported_orders_in_auction_orders(orders)
            .await
    }

    /// Removes all unsupported orders from the auction.
    pub async fn filter_unsupported_orders_in_auction_orders(&self, orders: &mut Vec<Order>) {
        let unsupported_uids = self.unsupported_order_uids(orders).await;
        // Filter out unsupported orders
        if !unsupported_uids.is_empty() {
            orders.retain(|order| !unsupported_uids.contains(&order.uid));
        }
    }

    /// Returns a set of orders uids for orders that are found to be
    /// unsupported.
    pub async fn unsupported_order_uids(&self, orders: &[Order]) -> HashSet<Uid> {
        let now = Instant::now();
        let mut token_quality_checks = FuturesUnordered::new();

        let mut removed_uids: HashSet<Uid> = orders
            .iter()
            .filter_map(|order| {
                // Check if uid is unsupported in metrics
                if matches!(
                    self.metrics
                        .as_ref()
                        .map(|m| m.get_quality(&order.uid, now)),
                    Some(Quality::Unsupported)
                ) {
                    return Some(order.uid);
                }
                let sell = self.get_token_quality(order.sell.token, now);
                let buy = self.get_token_quality(order.buy.token, now);
                match (sell, buy) {
                    // both tokens supported => keep order
                    (Quality::Supported, Quality::Supported) => None,
                    // at least 1 token unsupported => drop order
                    (Quality::Unsupported, _) | (_, Quality::Unsupported) => Some(order.uid),
                    // sell token quality is unknown => keep order if token is supported
                    (Quality::Unknown, _) => {
                        let Some(detector) = &self.simulation_detector else {
                            // we can't determine quality => assume order is good
                            return None;
                        };
                        let check_tokens_fut = async move {
                            let quality = detector.determine_sell_token_quality(order, now).await;
                            (order.uid, quality)
                        };
                        token_quality_checks.push(check_tokens_fut);
                        None
                    }
                    // buy token quality is unknown => keep order (because we can't
                    // determine quality and assume it's good)
                    (_, Quality::Unknown) => None,
                }
            })
            .collect();

        while let Some((uid, quality)) = token_quality_checks.next().await {
            if quality != Quality::Supported {
                removed_uids.insert(uid);
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
                    app_data::AppData,
                    signature,
                },
                risk_detector::bad_tokens::simulation::MockSellQualityDetector,
            },
            infra::solver,
            util,
        },
        app_data::{
            AppDataHash,
            Flashloan,
            ProtocolAppData,
            Root,
            ValidatedAppData,
            hash_full_app_data,
        },
        eth_domain_types::{Asset, TokenAmount, U256},
        std::{sync::Arc, time::Duration},
    };

    // Helper to create a mock eth:Address purely for test
    fn addr(n: u8) -> eth::Address {
        eth::Address::from_slice(&[n; 20])
    }

    // Helper to create a mock order purely for test
    fn order(
        uid: Uid,
        signer: eth::Address,
        sell_token: eth::TokenAddress,
        buy_token: eth::TokenAddress,
        valid_to: u32,
        flashloan: bool,
    ) -> Order {
        let app_data = if flashloan {
            let protocol = ProtocolAppData {
                flashloan: Some(Flashloan {
                    liquidity_provider: addr(99),
                    receiver: signer,
                    token: sell_token.into(),
                    protocol_adapter: addr(98),
                    amount: U256::from(1),
                }),
                ..Default::default()
            };
            let root = Root::new(Some(protocol.clone()));
            let document = serde_json::to_string(&root).unwrap();
            let hash = AppDataHash(hash_full_app_data(document.as_bytes()));
            AppData::Full(Arc::new(ValidatedAppData {
                hash,
                document,
                protocol,
            }))
        } else {
            Default::default()
        };

        Order {
            uid,
            receiver: Some(signer),
            created: util::Timestamp(0),
            valid_to: util::Timestamp(valid_to),
            buy: Asset {
                token: buy_token,
                amount: TokenAmount::from(1),
            },
            sell: Asset {
                token: sell_token,
                amount: TokenAmount::from(1),
            },
            side: Side::Sell,
            kind: Kind::Limit,
            app_data,
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

    // Helper to create a mock sell quality detector purely for test
    fn sell_quality_detector(unsupported_uid: Uid) -> MockSellQualityDetector {
        let mut detector = MockSellQualityDetector::new();
        detector
            .expect_determine_sell_token_quality()
            .returning(move |order, _now| {
                if order.uid == unsupported_uid {
                    Quality::Unsupported
                } else {
                    Quality::Supported
                }
            });
        detector
            .expect_get_quality()
            .returning(|_, _| Quality::Unknown);
        detector.expect_evict_outdated_entries().returning(|| ());
        detector
    }

    #[tokio::test]
    async fn without_unsupported_orders_returns_empty() {
        let mut orders = vec![];
        let detector = Detector::new(Default::default());
        detector.without_unsupported_orders(&mut orders, true).await;
        assert!(orders.is_empty());
    }

    #[tokio::test]
    async fn ithout_unsupported_orders_all_supported_orders_are_kept() {
        let signer = addr(1);
        let sell_token = addr(2).into();
        let buy_token = addr(3).into();
        let order_uid = uid(1, signer, u32::MAX);
        let mut orders = vec![order(
            order_uid,
            signer,
            sell_token,
            buy_token,
            u32::MAX,
            false,
        )];

        let detector = Detector::new(Default::default());

        detector.without_unsupported_orders(&mut orders, true).await;

        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].uid, order_uid);
    }

    #[tokio::test]
    async fn without_unsupported_orders_filters_unsupported_orders_and_flashloans() {
        let valid_to = u32::MAX;

        let metrics_bad_uid = uid(1, addr(6), valid_to);
        let token_bad_uid = uid(2, addr(7), valid_to);
        let supported_uid = uid(3, addr(8), valid_to);
        let unknown_sell_uid = uid(4, addr(9), valid_to);
        let unknown_buy_uid = uid(5, addr(10), valid_to);
        let sell_detector_unsupported_uid = uid(6, addr(11), valid_to);
        let sell_detector_supported_uid = uid(7, addr(12), valid_to);
        let flashloan_uid = uid(8, addr(13), valid_to);

        let metrics_detector = bad_orders::metrics::Detector::new(
            0.5,
            2,
            false,
            Duration::from_secs(60),
            Duration::from_secs(60),
            Duration::from_secs(60),
            solver::Name("test_solver".into()),
        );
        let metrics_quality_unsupported_address = addr(3).into();
        let mut detector_config = HashMap::new();
        detector_config.insert(metrics_quality_unsupported_address, Quality::Unsupported);

        let mut detector = Detector::new(detector_config);
        detector.with_metrics_detector(metrics_detector);

        // Simulate repeated metrics failure for order with metrics_bad_uid
        detector.encoding_failed(&[metrics_bad_uid]);
        detector.encoding_failed(&[metrics_bad_uid]);
        detector.encoding_failed(&[metrics_bad_uid]);
        detector.with_simulation_detector(sell_quality_detector(sell_detector_unsupported_uid));

        // Test with flashloans disabled
        let mut orders_flashloans_disabled = vec![
            order(
                metrics_bad_uid,
                addr(6),
                addr(1).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                token_bad_uid,
                addr(7),
                metrics_quality_unsupported_address,
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                supported_uid,
                addr(8),
                addr(1).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                unknown_sell_uid,
                addr(9),
                addr(4).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                unknown_buy_uid,
                addr(10),
                addr(1).into(),
                addr(5).into(),
                valid_to,
                false,
            ),
            order(
                sell_detector_unsupported_uid,
                addr(11),
                addr(6).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                sell_detector_supported_uid,
                addr(12),
                addr(7).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                flashloan_uid,
                addr(13),
                addr(1).into(),
                addr(2).into(),
                valid_to,
                true,
            ),
        ];

        detector
            .without_unsupported_orders(&mut orders_flashloans_disabled, false)
            .await;

        let remaining_uids = orders_flashloans_disabled
            .iter()
            .map(|order| order.uid)
            .collect::<Vec<_>>();

        assert_eq!(
            remaining_uids,
            vec![
                supported_uid,
                unknown_sell_uid,
                unknown_buy_uid,
                sell_detector_supported_uid,
            ]
        );

        // Test with flashloans enabled
        let mut orders_flashloans_enabled = vec![
            order(
                metrics_bad_uid,
                addr(6),
                addr(1).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                token_bad_uid,
                addr(7),
                addr(3).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                supported_uid,
                addr(8),
                addr(1).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                unknown_sell_uid,
                addr(9),
                addr(4).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                unknown_buy_uid,
                addr(10),
                addr(1).into(),
                addr(5).into(),
                valid_to,
                false,
            ),
            order(
                sell_detector_unsupported_uid,
                addr(11),
                addr(6).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                sell_detector_supported_uid,
                addr(12),
                addr(7).into(),
                addr(2).into(),
                valid_to,
                false,
            ),
            order(
                flashloan_uid,
                addr(13),
                addr(1).into(),
                addr(2).into(),
                valid_to,
                true,
            ),
        ];

        detector
            .without_unsupported_orders(&mut orders_flashloans_enabled, true)
            .await;

        let remaining_uids = orders_flashloans_enabled
            .iter()
            .map(|order| order.uid)
            .collect::<Vec<_>>();

        assert_eq!(
            remaining_uids,
            vec![
                supported_uid,
                unknown_sell_uid,
                unknown_buy_uid,
                sell_detector_supported_uid,
                flashloan_uid,
            ]
        );
    }
}

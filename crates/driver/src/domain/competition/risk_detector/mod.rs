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
        risk_detector::bad_tokens::simulation::DetectorApi,
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

enum OrderQuality {
    Supported,
    Unsupported,
    ShouldRequireSellTokenSimulation,
}

#[derive(Default)]
pub struct Detector {
    /// manually configured list of supported and unsupported tokens. Only
    /// tokens that get detected incorrectly by the automatic detectors get
    /// listed here and therefore have a higher precedence.
    hardcoded: HashMap<eth::TokenAddress, Quality>,
    simulation_detector: Option<Box<dyn DetectorApi>>,
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
    pub fn with_simulation_detector(&mut self, detector: impl DetectorApi + 'static) -> &mut Self {
        self.simulation_detector = Some(Box::new(detector));
        self
    }

    /// Enables detection of unsupported tokens based on heuristics.
    pub fn with_metrics_detector(&mut self, detector: bad_orders::metrics::Detector) -> &mut Self {
        self.metrics = Some(detector);
        self
    }

    /// Filters out unsupported and disallowed orders from the auction.
    #[instrument(skip_all)]
    pub async fn without_unsupported_orders(
        &self,
        orders: &mut Vec<Order>,
        flashloans_enabled: bool,
    ) {
        let now = Instant::now();
        // List of orders that have been removed
        let mut removed_uids: HashSet<Uid> = HashSet::new();
        // Choose filtering path depending on whether simulation detector is set
        let supported_orders = match &self.simulation_detector {
            Some(detector) => {
                let supported_orders = self
                    .supported_orders_with_detector(
                        orders.drain(..),
                        flashloans_enabled,
                        now,
                        &mut removed_uids,
                        detector.as_ref(),
                    )
                    .await;
                detector.evict_outdated_entries();
                supported_orders
            }
            None => self.supported_orders_without_detector(
                orders.drain(..),
                flashloans_enabled,
                now,
                &mut removed_uids,
            ),
        };
        if !removed_uids.is_empty() {
            tracing::debug!(orders = ?removed_uids, "ignored orders with unsupported tokens");
        }
        // Replace the original orders in the auction with supported orders
        *orders = supported_orders;
    }

    /// Filters orders using only static checks
    fn supported_orders_without_detector(
        &self,
        orders: impl Iterator<Item = Order>,
        flashloans_enabled: bool,
        now: Instant,
        removed_uids: &mut HashSet<Uid>,
    ) -> Vec<Order> {
        let mut supported_orders = Vec::new();
        for order in orders {
            // Flashloans are disabled => drop order
            if Self::is_disabled_flashloan_order(&order, flashloans_enabled) {
                removed_uids.insert(order.uid);
                continue;
            }
            // Determine whether to keep or drop order
            match self.order_quality(&order, now) {
                OrderQuality::Supported | OrderQuality::ShouldRequireSellTokenSimulation => {
                    supported_orders.push(order);
                }
                OrderQuality::Unsupported => {
                    removed_uids.insert(order.uid);
                }
            }
        }
        supported_orders
    }

    /// Filters orders and uses simulation for unknown sell-token quality.
    async fn supported_orders_with_detector(
        &self,
        orders: impl Iterator<Item = Order>,
        flashloans_enabled: bool,
        now: Instant,
        removed_uids: &mut HashSet<Uid>,
        detector: &dyn DetectorApi,
    ) -> Vec<Order> {
        let mut supported_orders = Vec::new();
        let mut orders_requiring_simulation = Vec::new();

        for order in orders {
            // Flashloans are disabled => drop order
            if Self::is_disabled_flashloan_order(&order, flashloans_enabled) {
                removed_uids.insert(order.uid);
                continue;
            }
            // Determine whether to keep, simulate or drop order
            match self.order_quality(&order, now) {
                OrderQuality::Supported => {
                    supported_orders.push(order);
                }
                OrderQuality::Unsupported => {
                    removed_uids.insert(order.uid);
                }
                OrderQuality::ShouldRequireSellTokenSimulation => {
                    orders_requiring_simulation.push(order);
                }
            }
        }

        // If no orders require simulation, return early
        if orders_requiring_simulation.is_empty() {
            return supported_orders;
        }

        let mut token_quality_checks = FuturesUnordered::new();
        for order in orders_requiring_simulation {
            // Add to quality checks to determine if supported or unsupported
            token_quality_checks.push(async move {
                let quality = detector.determine_sell_token_quality(&order, now).await;
                (order, quality)
            });
        }
        // Wait for all quality checks to complete
        while let Some((order, quality)) = token_quality_checks.next().await {
            if quality == Quality::Supported {
                supported_orders.push(order);
            } else {
                removed_uids.insert(order.uid);
            }
        }
        supported_orders
    }

    /// Classifies an order using metrics and static checks.
    fn order_quality(&self, order: &Order, now: Instant) -> OrderQuality {
        // Metrics determined quality is unsupported => drop order
        if self
            .metrics
            .as_ref()
            .map(|metrics| metrics.get_quality(&order.uid, now))
            .is_some_and(|q| q == Quality::Unsupported)
        {
            return OrderQuality::Unsupported;
        }
        let sell = self.get_token_quality(order.sell.token, now);
        let buy = self.get_token_quality(order.buy.token, now);
        match (sell, buy) {
            // both tokens supported => keep order
            (Quality::Supported, Quality::Supported) => OrderQuality::Supported,
            // at least 1 token unsupported => drop order
            (Quality::Unsupported, _) | (_, Quality::Unsupported) => OrderQuality::Unsupported,
            // sell token quality is unknown => should require simulation detector,
            // assume it is good if simulation detector is unavailable
            (Quality::Unknown, _) => OrderQuality::ShouldRequireSellTokenSimulation,
            // buy token quality is unknown => keep order (because we can't
            // determine quality and assume it's good)
            (_, Quality::Unknown) => OrderQuality::Supported,
        }
    }

    /// Returns true if flashloans are disabled and the order uses one.
    fn is_disabled_flashloan_order(order: &Order, flashloans_enabled: bool) -> bool {
        !flashloans_enabled && order.app_data.flashloan().is_some()
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

    /// Returns the quality of a token using hardcoded configuration or the
    /// simulation detector.
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
                risk_detector::bad_tokens::simulation::MockDetectorApi,
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
        std::{
            collections::{HashMap, HashSet},
            sync::Arc,
            time::Duration,
        },
    };

    // Helper to create a mock eth:Address purely for test
    fn addr(n: u8) -> eth::Address {
        eth::Address::from_slice(&[n; 20])
    }

    // Helper to create a mock UID purely for test
    fn uid(n: u8, signer: eth::Address, valid_to: u32) -> Uid {
        let order_hash = eth::B256::from([n; 32]);
        Uid::from_parts(order_hash, signer, valid_to)
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

    // Helper to create a mock detector purely for test
    fn detector(
        hardcoded_tokens: HashMap<eth_domain_types::TokenAddress, Quality>,
        metrics_bad_uids: HashSet<Uid>,
    ) -> Detector {
        let metrics_detector = bad_orders::metrics::Detector::new(
            0.5,
            2,
            false,
            Duration::from_secs(60),
            Duration::from_secs(60),
            Duration::from_secs(60),
            solver::Name("test_solver".into()),
        );

        let mut detector = Detector::new(hardcoded_tokens);
        detector.with_metrics_detector(metrics_detector);
        // Simulate multiple encoding failures to mark uid bad
        for uid in metrics_bad_uids {
            detector.encoding_failed(&[uid]);
            detector.encoding_failed(&[uid]);
            detector.encoding_failed(&[uid]);
        }

        detector
    }

    // Helper to create a mock simulation detector purely for test
    fn simulation_detector(unsupported_uids: HashSet<Uid>) -> MockDetectorApi {
        let mut detector = MockDetectorApi::new();
        detector
            .expect_determine_sell_token_quality()
            .returning(move |order, _| {
                if unsupported_uids.contains(&order.uid) {
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
        let detector = detector(HashMap::new(), HashSet::new());
        detector.without_unsupported_orders(&mut orders, true).await;
        assert!(orders.is_empty());
    }

    #[tokio::test]
    async fn without_unsupported_orders_flashloan_disabled_returns_empty() {
        let mut orders = vec![order(
            uid(1, addr(1), u32::MAX),
            addr(1),
            addr(1).into(),
            addr(2).into(),
            u32::MAX,
            true,
        )];
        let detector = detector(HashMap::new(), HashSet::new());
        detector
            .without_unsupported_orders(&mut orders, false)
            .await;
        assert!(orders.is_empty());
    }

    #[tokio::test]
    async fn without_unsupported_orders_flashloan_enabled_kept() {
        let id = uid(1, addr(1), u32::MAX);
        let mut orders = vec![order(
            id,
            addr(1),
            addr(1).into(),
            addr(2).into(),
            u32::MAX,
            true,
        )];
        let detector = detector(HashMap::new(), HashSet::new());

        // Note: This could be filtered out for other reasons later in reality
        // but is contained to be true for this test
        detector.without_unsupported_orders(&mut orders, true).await;
        assert_eq!(orders[0].uid, id);
    }

    #[tokio::test]
    async fn without_unsupported_orders_metrics_bad_returns_empty() {
        let unsupported_uid = uid(1, addr(1), u32::MAX);
        let mut orders = vec![order(
            unsupported_uid,
            addr(1),
            addr(1).into(),
            addr(2).into(),
            u32::MAX,
            false,
        )];
        let detector = detector(HashMap::new(), HashSet::from([unsupported_uid]));

        detector.without_unsupported_orders(&mut orders, true).await;
        assert!(orders.is_empty());
    }

    #[tokio::test]
    async fn without_unsupported_orders_supported_kept() {
        let uid = uid(1, addr(1), u32::MAX);
        let sell_token = addr(1).into();
        let buy_token = addr(2).into();
        let mut orders = vec![order(uid, addr(1), sell_token, buy_token, u32::MAX, false)];
        let detector = detector(
            HashMap::from([
                (sell_token, Quality::Supported),
                (buy_token, Quality::Supported),
            ]),
            HashSet::new(),
        );

        detector.without_unsupported_orders(&mut orders, true).await;
        assert_eq!(orders[0].uid, uid);
    }

    #[tokio::test]
    async fn without_unsupported_orders_buy_unsupported_empty() {
        let uid = uid(1, addr(1), u32::MAX);
        let sell_token = addr(1).into();
        let buy_token = addr(9).into();
        let mut orders = vec![order(uid, addr(1), sell_token, buy_token, u32::MAX, false)];
        let mut detector = detector(
            HashMap::from([(buy_token, Quality::Unsupported)]),
            HashSet::new(),
        );
        detector.with_simulation_detector(simulation_detector(HashSet::new()));

        detector.without_unsupported_orders(&mut orders, true).await;
        assert!(orders.is_empty());
    }

    #[tokio::test]
    async fn without_unsupported_orders_mixed_orders_filters_correctly() {
        let supported_uid = uid(1, addr(1), u32::MAX);
        let bad_token_uid = uid(2, addr(2), u32::MAX);
        let flashloan_uid = uid(3, addr(3), u32::MAX);
        let metrics_bad_uid = uid(4, addr(4), u32::MAX);
        let simulation_bad_uid = uid(5, addr(5), u32::MAX);

        let sell_token_supported = addr(1).into();
        let buy_token_supported = addr(2).into();
        let token_unsupported = addr(9).into();

        // For tracking and reasoning about orders in this test:
        //
        // order(
        //     // description -> expected outcome
        //     uid,        // if in metrics_bad_uids → discard
        //                 // can also be used to verify order
        //                 // inclusion/exclusion for the expected result.
        //     signer,     // unused
        //     sell_token, // checked in hardcoded map in detector:
        //                 //   Supported → continue
        //                 //   Unsupported → discard
        //                 //   Unknown → may trigger simulation if detector is set
        //     buy_token,  // checked in hardcoded map in detector:
        //                 //   Unsupported → discard
        //     valid_to,   // unused
        //     flashloan,  // if true and flashloans disabled → discard
        // )
        //
        // detector(
        //     HashMap::from([
        //         (sell_token_supported, Quality::Supported),
        //         (buy_token_supported, Quality::Supported),
        //         (token_unsupported, Quality::Unsupported),
        //     ]),
        //     HashSet::from([metrics_bad_uid]),
        // )
        //
        // simulation_detector(HashSet::from([simulation_bad_uid]))
        //     Runs only when sell_token quality is Unknown and a simulation detector
        //     is set. If uid is in provided set → discard.

        let mut orders = vec![
            order(
                // token supported -> keep
                supported_uid,
                addr(1),
                sell_token_supported,
                buy_token_supported,
                u32::MAX,
                false,
            ),
            order(
                // token unsupported -> discard
                bad_token_uid,
                addr(2),
                token_unsupported,
                buy_token_supported,
                u32::MAX,
                false,
            ),
            order(
                // flashloan -> discard, disabled
                flashloan_uid,
                addr(3),
                sell_token_supported,
                buy_token_supported,
                u32::MAX,
                true,
            ),
            order(
                // metrics bad -> discard
                metrics_bad_uid,
                addr(4),
                sell_token_supported,
                buy_token_supported,
                u32::MAX,
                false,
            ),
            order(
                // unsupported on quality check -> discard
                simulation_bad_uid,
                addr(5),
                addr(5).into(),
                buy_token_supported,
                u32::MAX,
                false,
            ),
        ];

        let mut detector = detector(
            HashMap::from([
                (sell_token_supported, Quality::Supported),
                (buy_token_supported, Quality::Supported),
                (token_unsupported, Quality::Unsupported),
            ]),
            HashSet::from([metrics_bad_uid]),
        );
        detector.with_simulation_detector(simulation_detector(HashSet::from([simulation_bad_uid])));

        detector
            .without_unsupported_orders(&mut orders, false)
            .await;

        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].uid, supported_uid);
    }
}

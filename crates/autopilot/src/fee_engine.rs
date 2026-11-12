//! The autopilot's fee policies from the rules of `[fee-engine]`, served while
//! they agree with the legacy fee policies.

use {
    crate::{
        boundary,
        domain::{
            self,
            fee::{
                AutopilotFees,
                Origin,
                Policy,
                PolicyWithOrigin,
                ProtocolFees,
                Quote,
                reference_quote,
            },
        },
    },
    alloy::primitives::Address,
    anyhow::Context,
    chrono::{DateTime, Utc},
    configs::{fee_engine::FeeEngineConfig, fee_factor::FeeFactor},
    fee_engine::{Evaluation, Fee, FeeSource, Outcome, RuleSet, RulesToml},
    std::sync::Arc,
};

/// The legacy fee policies without `[fee-engine]`; with it, the compared ones.
/// The rules must load.
pub fn protocol_fees(
    config: Option<&FeeEngineConfig>,
    legacy: ProtocolFees,
    native_token: Address,
    surplus_capturing_jit_order_owners: &[Address],
) -> anyhow::Result<Arc<dyn AutopilotFees>> {
    let Some(config) = config else {
        return Ok(Arc::new(legacy));
    };
    let engine = FeeEngineFees::new(config, native_token, surplus_capturing_jit_order_owners)?;
    Ok(Arc::new(ComparedFees::new(legacy, engine)))
}

/// The list the autopilot sets to the chain's wrapped native token.
const WRAPPED_NATIVE: &str = "wrapped_native";
/// The list the autopilot sets to the configured surplus capturing JIT order
/// owners.
const JIT_OWNERS: &str = "jit_owners";

/// The fees of the first rule that matches an order, then the order's capped
/// partner fees.
pub struct FeeEngineFees {
    rules: RuleSet,
}

impl FeeEngineFees {
    /// Loads the rules of `[fee-engine]`, with the lists `wrapped_native` and
    /// `jit_owners` added, which the section may not set.
    pub fn new(
        config: &FeeEngineConfig,
        native_token: Address,
        surplus_capturing_jit_order_owners: &[Address],
    ) -> anyhow::Result<Self> {
        let mut tables = RulesToml {
            lists: config.lists.clone(),
            rules: config.rules.clone(),
            partner_fees: config.partner_fees.clone(),
        };
        tables.add_host_list(WRAPPED_NATIVE, [native_token])?;
        tables.add_host_list(JIT_OWNERS, surplus_capturing_jit_order_owners)?;
        let rules = fee_engine::load_rules_toml(tables).context("loading the fee engine rules")?;
        let names: Vec<_> = rules.rules().iter().map(|rule| rule.name()).collect();
        tracing::info!(rules = ?names, "fee engine rules loaded");
        Ok(Self { rules })
    }

    fn evaluate(
        &self,
        order: &boundary::Order,
        in_market: bool,
        now: DateTime<Utc>,
    ) -> Evaluation<'_> {
        // TODO: keep the parsed documents by their hash; the parse is about
        // 1 µs per order, a quarter of the evaluation.
        // An order without a document that parses has no partner fee.
        let (app_data, partner_fees) = order
            .metadata
            .full_app_data
            .as_deref()
            .and_then(|document| {
                let protocol = app_data::parse(document.as_bytes()).ok()?;
                let app_code = app_data::app_code(document.as_bytes());
                Some((
                    shared::fee_engine::app_data(&protocol, app_code),
                    shared::fee_engine::partner_fees(&protocol),
                ))
            })
            .unwrap_or_default();
        let evaluation = self.rules.evaluator(now).evaluate(fee_engine::OrderView {
            order,
            app_data: &app_data,
            partner_fees: &partner_fees,
            in_market,
        });
        evaluation.warn_skipped();
        evaluation
    }

    /// The fee policies of an order in an auction.
    fn auction_fees(
        &self,
        order: &boundary::Order,
        quote: Option<&domain::Quote>,
        now: DateTime<Utc>,
    ) -> (Evaluation<'_>, Vec<PolicyWithOrigin>) {
        let quote = reference_quote(order, quote);
        let in_market = !boundary::is_order_outside_market_price(
            &order.into(),
            &(&quote).into(),
            order.data.kind,
        );
        let evaluation = self.evaluate(order, in_market, now);
        let fees = policies(&evaluation, &quote);
        (evaluation, fees)
    }

    /// The volume fee policies of a fast-path order: as the legacy source,
    /// the volume ones among its auction fees without a quote.
    fn fast_path_fees(
        &self,
        order: &boundary::Order,
        now: DateTime<Utc>,
    ) -> (Evaluation<'_>, Vec<PolicyWithOrigin>) {
        let (evaluation, mut fees) = self.auction_fees(order, None, now);
        fees.retain(|fee| matches!(fee.policy, Policy::Volume { .. }));
        (evaluation, fees)
    }
}

/// Which fee policy decision of the autopilot the engine was compared on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    /// The fee policies of an order in an auction.
    Auction,
    /// The volume fee policies of a fast-path order.
    FastPath,
}

impl Check {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auction => "auction",
            Self::FastPath => "fast_path",
        }
    }
}

/// The rules' fee policies while they are the legacy ones; on a disagreement,
/// the legacy ones, with the disagreement reported.
pub struct ComparedFees {
    legacy: ProtocolFees,
    engine: FeeEngineFees,
}

impl ComparedFees {
    fn new(legacy: ProtocolFees, engine: FeeEngineFees) -> Self {
        fee_engine::reset_comparison_counters(&[Check::Auction.as_str(), Check::FastPath.as_str()]);
        Self { legacy, engine }
    }

    fn auction_outcome(
        &self,
        order: &boundary::Order,
        quote: Option<&domain::Quote>,
        surplus_capturing_jit_order_owners: &[Address],
        now: DateTime<Utc>,
    ) -> (Vec<PolicyWithOrigin>, Outcome) {
        let check = Check::Auction.as_str();
        let legacy = fee_engine::time_evaluation(check, FeeSource::Legacy, || {
            self.legacy
                .auction_fees(order, quote, surplus_capturing_jit_order_owners, now)
        });
        let (evaluation, engine) = fee_engine::time_evaluation(check, FeeSource::Engine, || {
            self.engine.auction_fees(order, quote, now)
        });
        let outcome = report(Check::Auction, order, &evaluation, &engine, &legacy);
        (outcome.fees(engine, legacy), outcome)
    }

    fn fast_path_outcome(
        &self,
        order: &boundary::Order,
        surplus_capturing_jit_order_owners: &[Address],
        now: DateTime<Utc>,
    ) -> (Vec<PolicyWithOrigin>, Outcome) {
        let check = Check::FastPath.as_str();
        let legacy = fee_engine::time_evaluation(check, FeeSource::Legacy, || {
            self.legacy
                .fast_path_volume_fees(order, surplus_capturing_jit_order_owners, now)
        });
        let (evaluation, engine) = fee_engine::time_evaluation(check, FeeSource::Engine, || {
            self.engine.fast_path_fees(order, now)
        });
        let outcome = report(Check::FastPath, order, &evaluation, &engine, &legacy);
        (outcome.fees(engine, legacy), outcome)
    }
}

impl AutopilotFees for ComparedFees {
    fn auction_fees(
        &self,
        order: &boundary::Order,
        quote: Option<&domain::Quote>,
        surplus_capturing_jit_order_owners: &[Address],
        now: DateTime<Utc>,
    ) -> Vec<PolicyWithOrigin> {
        self.auction_outcome(order, quote, surplus_capturing_jit_order_owners, now)
            .0
    }

    fn fast_path_volume_fees(
        &self,
        order: &boundary::Order,
        surplus_capturing_jit_order_owners: &[Address],
        now: DateTime<Utc>,
    ) -> Vec<PolicyWithOrigin> {
        self.fast_path_outcome(order, surplus_capturing_jit_order_owners, now)
            .0
    }
}

/// Logs the fee policies if they disagree, and counts the outcome.
fn report(
    check: Check,
    order: &boundary::Order,
    evaluation: &Evaluation,
    engine: &[PolicyWithOrigin],
    legacy: &[PolicyWithOrigin],
) -> Outcome {
    // A skipped rule may have charged another fee.
    let agree =
        evaluation.skipped.is_empty() && engine.iter().map(in_bps).eq(legacy.iter().map(in_bps));
    fee_engine::report_comparison(check.as_str(), agree, || fee_engine::Mismatch {
        group: format!(
            "expected_bps={:?} got_bps={:?} rule={:?} skipped={:?}",
            legacy.iter().map(in_bps).collect::<Vec<_>>(),
            engine.iter().map(in_bps).collect::<Vec<_>>(),
            evaluation
                .matched
                .first()
                .map(|matched| matched.rule.name()),
            evaluation
                .skipped
                .iter()
                .map(|skipped| skipped.rule.name())
                .collect::<Vec<_>>(),
        ),
        example: format!(
            "order_uid={} sell_token={:?} buy_token={:?} kind={:?} owner={:?}",
            order.metadata.uid,
            order.data.sell_token,
            order.data.buy_token,
            order.data.kind,
            order.metadata.owner,
        ),
    })
}

/// The fees of the first rule that matched, the one the engine charges, then
/// the partner fees. `quote` is the reference of price improvement fees.
fn policies(evaluation: &Evaluation, quote: &domain::Quote) -> Vec<PolicyWithOrigin> {
    let protocol = evaluation
        .matched
        .first()
        .map(|matched| matched.fees.as_slice())
        .unwrap_or_default();
    let with_origin = |fees: &[Fee], origin| {
        fees.iter()
            .map(|fee| PolicyWithOrigin {
                policy: policy(*fee, quote),
                origin,
            })
            .collect::<Vec<_>>()
    };
    let mut policies = with_origin(protocol, Origin::Protocol);
    policies.extend(with_origin(&evaluation.partner_fees, Origin::Partner));
    policies
}

fn policy(fee: Fee, quote: &domain::Quote) -> Policy {
    let factor = |factor: fee_engine::FeeFactor| FeeFactor::new(factor.to_f64());
    match fee {
        Fee::Volume { factor: volume } => Policy::Volume {
            factor: factor(volume),
        },
        Fee::Surplus {
            factor: surplus,
            max_volume_factor,
        } => Policy::Surplus {
            factor: factor(surplus),
            max_volume_factor: factor(max_volume_factor),
        },
        Fee::PriceImprovement {
            factor: price_improvement,
            max_volume_factor,
        } => Policy::PriceImprovement {
            factor: factor(price_improvement),
            max_volume_factor: factor(max_volume_factor),
            quote: Quote::from_domain(quote),
        },
    }
}

/// What the two sources must share of a policy. Factors are in basis points
/// on the grid the fee math uses, so float noise is not a mismatch.
fn in_bps(fee: &PolicyWithOrigin) -> (Origin, &'static str, f64, Option<f64>) {
    let bps = |factor: &FeeFactor| factor.to_high_precision() as f64 / 100.;
    let (kind, factor, max_volume_factor) = match &fee.policy {
        Policy::Volume { factor } => ("volume", factor, None),
        Policy::Surplus {
            factor,
            max_volume_factor,
        } => ("surplus", factor, Some(max_volume_factor)),
        Policy::PriceImprovement {
            factor,
            max_volume_factor,
            // Both sources take the quote from `reference_quote`.
            ..
        } => ("price_improvement", factor, Some(max_volume_factor)),
    };
    (fee.origin, kind, bps(factor), max_volume_factor.map(bps))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{U256, address},
        configs::autopilot::fee_policy::FeePoliciesConfig,
        model::order::{BUY_ETH_ADDRESS, OrderData, OrderKind, OrderMetadata},
        shared::arguments::TokenBucketFeeOverride,
    };

    const WETH: Address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    const STETH: Address = address!("ae7ab96520de3a18e5e111b5eaab095312d7fe84");
    const USDC: Address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    const USDT: Address = address!("dac17f958d2ee523a2206206994597c13d831ec7");
    const DAI: Address = address!("6b175474e89094c44da98b954eedeac495271d0f");
    const JIT_OWNER: Address = address!("1111111111111111111111111111111111111111");

    const PAIRS: [(Address, Address); 8] = [
        (USDC, USDT),
        (WETH, STETH),
        (STETH, WETH),
        (STETH, BUY_ETH_ADDRESS),
        (WETH, BUY_ETH_ADDRESS),
        (USDC, USDC),
        (USDC, WETH),
        (DAI, USDC),
    ];

    /// The fee policies prod configures, with `volume` as the default volume
    /// fee.
    fn legacy_config(volume: f64) -> FeePoliciesConfig {
        toml::from_str(&format!(
            r#"
max-partner-fee = 0.05

[[policies]]
kind.price-improvement = {{ factor = 0.5, max-volume-factor = 0.0098 }}
order-class = "market"

[[policies]]
kind.surplus = {{ factor = 0.5, max-volume-factor = 0.0098 }}
order-class = "limit"

[[policies]]
kind.volume = {{ factor = {volume} }}
order-class = "any"
"#
        ))
        .unwrap()
    }

    /// `legacy_config(0.0002)` and `prod_buckets` as rules, the way prod
    /// configures them: a list and a rule per bucket.
    const RULES: &str = r#"
[partner-fees]
max-volume-bps = 500

[lists]
stablecoins = ["0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", "0xdac17f958d2ee523a2206206994597c13d831ec7"]
eth_correlated = ["0xae7ab96520de3a18e5e111b5eaab095312d7fe84", "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"]

[[rule]]
name = "JIT owners: no protocol fee"
when = "order.owner in jit_owners"
fee = "no_fee()"

[[rule]]
name = "Same token (incl. wrapped native -> native): no volume fee"
when = 'order.sellToken == order.buyToken || (order.sellToken in wrapped_native && order.buyToken == "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")'
fee = "order.inMarket ? price_improvement(50, 98) : surplus(50, 98)"

[[rule]]
name = "Stablecoins: 0.3 bps"
when = "order.sellToken in stablecoins && order.buyToken in stablecoins"
fee = "[order.inMarket ? price_improvement(50, 98) : surplus(50, 98), volume(0.3)]"

[[rule]]
name = "ETH-correlated: 0.3 bps"
when = "order.sellToken in eth_correlated && order.buyToken in eth_correlated"
fee = "[order.inMarket ? price_improvement(50, 98) : surplus(50, 98), volume(0.3)]"

[[rule]]
name = "Default: 2 bps"
when = "true"
fee = "[order.inMarket ? price_improvement(50, 98) : surplus(50, 98), volume(2)]"
"#;

    fn factor(value: f64) -> FeeFactor {
        FeeFactor::try_from(value).unwrap()
    }

    fn bucket(tokens: &[Address]) -> TokenBucketFeeOverride {
        TokenBucketFeeOverride {
            tokens: tokens.iter().copied().collect(),
            factor: factor(0.00003),
        }
    }

    fn prod_buckets() -> Vec<TokenBucketFeeOverride> {
        vec![bucket(&[USDC, USDT]), bucket(&[STETH, BUY_ETH_ADDRESS])]
    }

    fn legacy(volume: f64) -> ProtocolFees {
        ProtocolFees::new(&legacy_config(volume), prod_buckets(), false, WETH)
    }

    fn config() -> FeeEngineConfig {
        toml::from_str(RULES).unwrap()
    }

    fn engine() -> FeeEngineFees {
        FeeEngineFees::new(&config(), WETH, &[JIT_OWNER]).unwrap()
    }

    fn compared(volume: f64) -> ComparedFees {
        ComparedFees::new(legacy(volume), engine())
    }

    /// Sells 1000 `sell_token` for 900 `buy_token` or more.
    fn order(sell_token: Address, buy_token: Address, document: Option<&str>) -> boundary::Order {
        boundary::Order {
            data: OrderData {
                sell_token,
                buy_token,
                sell_amount: U256::from(1000),
                buy_amount: U256::from(900),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            metadata: OrderMetadata {
                full_app_data: document.map(str::to_owned),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// A quote that buys `buy_amount` for the 1000 sold of `order`.
    fn quote(buy_amount: u64) -> domain::Quote {
        domain::Quote {
            sell_amount: U256::from(1000).into(),
            buy_amount: U256::from(buy_amount).into(),
            ..Default::default()
        }
    }

    fn volume(value: f64, origin: Origin) -> PolicyWithOrigin {
        PolicyWithOrigin {
            policy: Policy::Volume {
                factor: factor(value),
            },
            origin,
        }
    }

    const VOLUME_PARTNER_FEES: &str = r#"{"appCode": "CoW Swap", "metadata": {"partnerFee": [
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 400},
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 300}
    ]}}"#;
    const SURPLUS_THEN_VOLUME_PARTNER_FEES: &str = r#"{"metadata": {"partnerFee": [
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "surplusBps": 50, "maxVolumeBps": 480},
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "priceImprovementBps": 50, "maxVolumeBps": 10},
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 50}
    ]}}"#;
    const DOCUMENTS: [Option<&str>; 5] = [
        None,
        Some("{}"),
        Some("not a document"),
        Some(VOLUME_PARTNER_FEES),
        Some(SURPLUS_THEN_VOLUME_PARTNER_FEES),
    ];

    #[test]
    fn agrees_with_the_legacy_fee_policies() {
        let compared = compared(0.0002);
        // None, inside the market price and outside of it.
        let quotes = [None, Some(quote(2000)), Some(quote(500))];
        for (sell, buy) in PAIRS {
            for document in DOCUMENTS {
                for quote in &quotes {
                    let order = order(sell, buy, document);
                    let (fees, outcome) =
                        compared.auction_outcome(&order, quote.as_ref(), &[], Utc::now());
                    assert_eq!(
                        outcome,
                        Outcome::Match,
                        "{sell:?} -> {buy:?}, {document:?}, {quote:?}: {fees:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn auction_fees_are_the_protocol_fees_then_the_capped_partner_fees() {
        let order = order(USDC, WETH, Some(VOLUME_PARTNER_FEES));
        let quote = quote(2000);
        let (_, fees) = engine().auction_fees(&order, Some(&quote), Utc::now());
        assert_eq!(
            fees,
            [
                PolicyWithOrigin {
                    policy: Policy::PriceImprovement {
                        factor: factor(0.5),
                        max_volume_factor: factor(0.0098),
                        quote: Quote::from_domain(&quote),
                    },
                    origin: Origin::Protocol,
                },
                volume(0.0002, Origin::Protocol),
                volume(0.04, Origin::Partner),
                // What the 5% cap leaves after the 4%.
                volume(0.009615384615384616, Origin::Partner),
            ]
        );
    }

    #[test]
    fn a_skipped_rule_is_a_mismatch() {
        // The first rule fails on the order (sell amount 1000) only; the
        // default then charges the legacy fees.
        let config = toml::from_str(
            r#"
[[rule]]
name = "x"
when = "order.sellAmount == 1000.0 ? order.missing == 1 : false"
fee = "volume(1)"

[[rule]]
name = "y"
when = "true"
fee = "[order.inMarket ? price_improvement(50, 98) : surplus(50, 98), volume(2)]"
"#,
        )
        .unwrap();
        let engine = FeeEngineFees::new(&config, WETH, &[]).unwrap();
        let order = order(USDC, WETH, None);
        let (_, engine_fees) = engine.auction_fees(&order, None, Utc::now());
        let (fees, outcome) = ComparedFees::new(legacy(0.0002), engine).auction_outcome(
            &order,
            None,
            &[],
            Utc::now(),
        );
        assert_eq!(fees, engine_fees);
        assert_eq!(outcome, Outcome::Mismatch);
    }

    #[test]
    fn serves_the_engine_fees_on_a_match_else_the_legacy_ones() {
        // The rules charge 2 bps by default.
        let serve = |config: Option<&FeeEngineConfig>, legacy_volume: f64| {
            protocol_fees(config, legacy(legacy_volume), WETH, &[])
                .unwrap()
                .auction_fees(&order(USDC, WETH, None), None, &[], Utc::now())[1]
        };
        let config = config();
        // 2 bps on the grid, so a match; the factor served is the engine's.
        let noisy = 0.000_200_000_1;
        assert_eq!(serve(None, noisy), volume(noisy, Origin::Protocol));
        assert_eq!(
            serve(Some(&config), noisy),
            volume(0.0002, Origin::Protocol)
        );
        assert_eq!(
            serve(Some(&config), 0.0003),
            volume(0.0003, Origin::Protocol)
        );
    }

    #[test]
    fn rules_must_load() {
        let broken =
            toml::from_str("[[rule]]\nname = 'x'\nwhen = 'true'\nfee = 'volume('\n").unwrap();
        assert!(protocol_fees(Some(&broken), legacy(0.0002), WETH, &[]).is_err());
    }

    #[test]
    fn jit_owners_pay_only_partner_fees() {
        let compared = compared(0.0002);
        let mut order = order(USDC, WETH, Some(VOLUME_PARTNER_FEES));
        order.metadata.owner = JIT_OWNER;
        let (fees, outcome) = compared.auction_outcome(&order, None, &[JIT_OWNER], Utc::now());
        assert_eq!(outcome, Outcome::Match);
        assert_eq!(
            fees,
            [
                volume(0.04, Origin::Partner),
                volume(0.009615384615384616, Origin::Partner),
            ]
        );
        // The rules read the configured owners; the legacy source reads the
        // owners of the auction, which leave out the ones with an order in
        // flight.
        let (fees, outcome) = compared.auction_outcome(&order, None, &[], Utc::now());
        assert_eq!(outcome, Outcome::Mismatch);
        assert_eq!(fees.len(), 4);
    }

    #[test]
    fn fast_path_agrees_on_the_volume_fees() {
        let compared = compared(0.0002);
        for (sell, buy) in PAIRS {
            for document in DOCUMENTS {
                let (fees, outcome) =
                    compared.fast_path_outcome(&order(sell, buy, document), &[], Utc::now());
                assert_eq!(
                    outcome,
                    Outcome::Match,
                    "{sell:?} -> {buy:?}, {document:?}: {fees:?}"
                );
            }
        }
        let (fees, _) = compared.fast_path_outcome(
            &order(USDC, WETH, Some(VOLUME_PARTNER_FEES)),
            &[],
            Utc::now(),
        );
        assert_eq!(
            fees,
            [
                volume(0.0002, Origin::Protocol),
                volume(0.04, Origin::Partner),
                volume(0.009615384615384616, Origin::Partner),
            ]
        );
    }

    #[test]
    fn fast_path_partner_surplus_fees_take_from_the_cap_as_in_the_legacy_source() {
        let order = order(USDC, WETH, Some(SURPLUS_THEN_VOLUME_PARTNER_FEES));
        let (fees, outcome) = compared(0.0002).fast_path_outcome(&order, &[], Utc::now());
        assert_eq!(outcome, Outcome::Match);
        // What the 5% cap leaves after the 4.8% and the 0.1%.
        assert_eq!(
            fees,
            [
                volume(0.0002, Origin::Protocol),
                volume(0.0009532888465204958, Origin::Partner),
            ]
        );
    }
}

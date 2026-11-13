//! The orderbook's volume fees from the rules of `[fee-engine]`, served while
//! they agree with the legacy volume fees.

use {
    alloy::primitives::Address,
    anyhow::Context,
    app_data::ValidatedAppData,
    chrono::Utc,
    configs::{fee_factor::FeeFactor, orderbook::FeeEngineConfig},
    fee_engine::{Evaluation, Fee, Outcome, PartnerFee, RuleSet, RulesToml},
    model::order::{Order, OrderUid},
    shared::{
        fee::{LegacyVolumeFees, OrderbookFees},
        order_quoting::Quote,
        order_validation::{Amounts, is_order_outside_market_price},
    },
    std::sync::Arc,
};

/// The legacy volume fees without `[fee-engine]`; with it, the compared ones.
/// The rules must load.
pub fn volume_fees(
    config: Option<&FeeEngineConfig>,
    legacy: LegacyVolumeFees,
    native_token: Address,
) -> anyhow::Result<Arc<dyn OrderbookFees>> {
    let Some(config) = config else {
        return Ok(Arc::new(legacy));
    };
    let engine = FeeEngineVolumeFees::new(config, native_token)?;
    Ok(Arc::new(ComparedVolumeFees::new(legacy, engine)))
}

/// The list the orderbook sets to the chain's wrapped native token.
const WRAPPED_NATIVE: &str = "wrapped_native";

/// The volume fees among the fees the rules charge. The orderbook needs no
/// other: surplus and price improvement fees take nothing at the quoted price.
pub struct FeeEngineVolumeFees {
    rules: RuleSet,
}

impl FeeEngineVolumeFees {
    /// Loads the rules of `[fee-engine]`, with the chain's wrapped native
    /// token added as the list `wrapped_native`, which the section may not set.
    pub fn new(config: &FeeEngineConfig, native_token: Address) -> anyhow::Result<Self> {
        let mut lists = config
            .lists
            .clone()
            .unwrap_or_else(|| toml::Table::new().into());
        // Lists that are not a table are left for the loader to refuse.
        if let Some(lists) = lists.as_table_mut() {
            anyhow::ensure!(
                !lists.contains_key(WRAPPED_NATIVE),
                "[fee-engine] may not set the list `{WRAPPED_NATIVE}`"
            );
            lists.insert(
                WRAPPED_NATIVE.to_owned(),
                vec![native_token.to_string()].into(),
            );
        }
        let rules = fee_engine::load_rules_toml(RulesToml {
            lists: Some(lists),
            rules: config.rules.clone(),
            partner_fees: config.partner_fees.clone(),
        })
        .context("loading the fee engine rules")?;
        let names: Vec<_> = rules.rules().iter().map(|rule| rule.name()).collect();
        tracing::info!(rules = ?names, "fee engine rules loaded");
        Ok(Self { rules })
    }

    fn evaluate(
        &self,
        order: &Order,
        app_data: &ValidatedAppData,
        app_code: Option<String>,
        partner_fees: &[PartnerFee],
        in_market: bool,
    ) -> Evaluation<'_> {
        let evaluation = self
            .rules
            .evaluator(Utc::now())
            .evaluate(fee_engine::OrderView {
                order,
                app_data: &shared::fee_engine::app_data(&app_data.protocol, app_code),
                partner_fees,
                in_market,
            });
        evaluation.warn_skipped();
        evaluation
    }

    /// The protocol volume fees for the order a quote describes.
    fn quote_fees(
        &self,
        order: &Order,
        app_data: &ValidatedAppData,
        app_code: Option<&str>,
    ) -> (Evaluation<'_>, Vec<FeeFactor>) {
        // The quote leaves partner fees out and sets the market price.
        let app_code = app_code.map(str::to_owned);
        let evaluation = self.evaluate(order, app_data, app_code, &[], true);
        let fees = protocol_volume_fees(&evaluation);
        (evaluation, fees)
    }

    /// The protocol, then the capped partner, volume fees of a fast-path order.
    fn fast_path_fees(
        &self,
        order: &Order,
        quote: &Quote,
        app_data: &ValidatedAppData,
    ) -> (Evaluation<'_>, Vec<FeeFactor>) {
        let in_market = !is_order_outside_market_price(
            &Amounts {
                sell: order.data.sell_amount,
                buy: order.data.buy_amount,
                fee: order.data.fee_amount,
            },
            &Amounts {
                sell: quote.sell_amount,
                buy: quote.buy_amount,
                fee: quote.fee_amount,
            },
            order.data.kind,
        );
        let partner_fees = shared::fee_engine::fast_path_partner_fees(&app_data.protocol);
        // Placement reads the app code nowhere else, so this parse is its only
        // one.
        let app_code = app_data::app_code(app_data.document.as_bytes());
        let evaluation = self.evaluate(order, app_data, app_code, &partner_fees, in_market);
        let mut fees = protocol_volume_fees(&evaluation);
        fees.extend(volume_factors(&evaluation.partner_fees));
        (evaluation, fees)
    }
}

/// Which volume fee decision of the orderbook the engine was compared on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    /// The volume fee taken out of a quote.
    Quote,
    /// The volume fees the fast-path limit check leaves room for.
    FastPath,
}

impl Check {
    fn as_str(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::FastPath => "fast_path",
        }
    }
}

/// The rules' volume fees while they are the legacy ones; on a disagreement,
/// the legacy ones, with the disagreement reported.
pub struct ComparedVolumeFees {
    legacy: LegacyVolumeFees,
    engine: FeeEngineVolumeFees,
}

impl ComparedVolumeFees {
    fn new(legacy: LegacyVolumeFees, engine: FeeEngineVolumeFees) -> Self {
        fee_engine::reset_comparison_counters(&[Check::Quote.as_str(), Check::FastPath.as_str()]);
        Self { legacy, engine }
    }

    fn quote_outcome(
        &self,
        order: &Order,
        quote: &Quote,
        app_data: &ValidatedAppData,
        app_code: Option<&str>,
    ) -> (Option<FeeFactor>, Outcome) {
        let legacy = self
            .legacy
            .quote_volume_fee(order, quote, app_data, app_code);
        let (evaluation, engine) = self.engine.quote_fees(order, app_data, app_code);
        let legacy = Vec::from_iter(legacy);
        let outcome = report(Check::Quote, quote.id, order, &evaluation, &engine, &legacy);
        // A match has as many factors as the legacy fees: one at most.
        (outcome.fees(engine, legacy).first().copied(), outcome)
    }

    fn fast_path_outcome(
        &self,
        order: &Order,
        quote: &Quote,
        app_data: &ValidatedAppData,
    ) -> (Vec<FeeFactor>, Outcome) {
        let legacy = self.legacy.fast_path_volume_fees(order, quote, app_data);
        let (evaluation, engine) = self.engine.fast_path_fees(order, quote, app_data);
        let outcome = report(
            Check::FastPath,
            quote.id,
            order,
            &evaluation,
            &engine,
            &legacy,
        );
        (outcome.fees(engine, legacy), outcome)
    }
}

impl OrderbookFees for ComparedVolumeFees {
    fn quote_volume_fee(
        &self,
        order: &Order,
        quote: &Quote,
        app_data: &ValidatedAppData,
        app_code: Option<&str>,
    ) -> Option<FeeFactor> {
        self.quote_outcome(order, quote, app_data, app_code).0
    }

    fn fast_path_volume_fees(
        &self,
        order: &Order,
        quote: &Quote,
        app_data: &ValidatedAppData,
    ) -> Vec<FeeFactor> {
        self.fast_path_outcome(order, quote, app_data).0
    }
}

/// Logs the volume fees if they disagree, and counts the outcome.
fn report(
    check: Check,
    quote_id: Option<model::quote::QuoteId>,
    order: &Order,
    evaluation: &Evaluation,
    engine: &[FeeFactor],
    legacy: &[FeeFactor],
) -> Outcome {
    // A skipped rule may have charged another fee.
    let agree = evaluation.skipped.is_empty()
        && match check {
            // `protocolFeeBps` shows a zero factor, so it must match too.
            Check::Quote => grid(engine).eq(grid(legacy)),
            // A zero factor leaves the limit unchanged, like no factor.
            Check::FastPath => {
                let non_zero = |factors| grid(factors).filter(|&factor| factor != 0);
                non_zero(engine).eq(non_zero(legacy))
            }
        };
    fee_engine::report_comparison(check.as_str(), agree, || {
        let matched = evaluation.matched.first();
        let order_uid = Some(order.metadata.uid).filter(|uid| *uid != OrderUid::default());
        fee_engine::Mismatch {
            group: format!(
                "expected_bps={:?} got_bps={:?} rule={:?} rule_fees={:?} skipped={:?}",
                bps(legacy),
                bps(engine),
                matched.map(|matched| matched.rule.name()),
                matched.map(|matched| &matched.fees),
                evaluation
                    .skipped
                    .iter()
                    .map(|skipped| skipped.rule.name())
                    .collect::<Vec<_>>(),
            ),
            example: format!(
                "quote_id={quote_id:?} order_uid={order_uid:?} sell_token={:?} buy_token={:?} \
                 kind={:?} owner={:?}",
                order.data.sell_token, order.data.buy_token, order.data.kind, order.metadata.owner,
            ),
        }
    })
}

/// The volume fees of the first rule that matched, the one the engine charges.
fn protocol_volume_fees(evaluation: &Evaluation) -> Vec<FeeFactor> {
    evaluation
        .matched
        .first()
        .map(|matched| volume_factors(&matched.fees))
        .unwrap_or_default()
}

fn volume_factors(fees: &[Fee]) -> Vec<FeeFactor> {
    fees.iter()
        .filter_map(|fee| match fee {
            Fee::Volume { factor } => Some(FeeFactor::new(factor.to_f64())),
            Fee::Surplus { .. } | Fee::PriceImprovement { .. } => None,
        })
        .collect()
}

/// Factors in basis points, as `protocolFeeBps` shows them.
fn bps(factors: &[FeeFactor]) -> Vec<String> {
    factors.iter().map(FeeFactor::to_bps_str).collect()
}

/// Factors on the grid the fee math uses, so float noise is not a mismatch.
fn grid(factors: &[FeeFactor]) -> impl Iterator<Item = u64> {
    factors.iter().map(|factor| factor.to_high_precision())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{U256, address},
        configs::orderbook::VolumeFeeConfig,
        fee_engine::{PartnerFees, RuleDefinition, RuleSetDefinition},
        model::order::{BUY_ETH_ADDRESS, OrderData, OrderKind},
        shared::{
            arguments::TokenBucketFeeOverride,
            fee::VolumeFeePolicy,
            order_quoting::QuoteData,
        },
        std::collections::BTreeMap,
    };

    const WETH: Address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    const STETH: Address = address!("ae7ab96520de3a18e5e111b5eaab095312d7fe84");
    const USDC: Address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    const USDT: Address = address!("dac17f958d2ee523a2206206994597c13d831ec7");
    const DAI: Address = address!("6b175474e89094c44da98b954eedeac495271d0f");

    const PAIRS: [(Address, Address); 11] = [
        (USDC, USDT),
        (WETH, STETH),
        (STETH, WETH),
        (STETH, BUY_ETH_ADDRESS),
        (BUY_ETH_ADDRESS, STETH),
        (WETH, BUY_ETH_ADDRESS),
        (USDC, USDC),
        (USDC, WETH),
        (WETH, USDC),
        (USDC, BUY_ETH_ADDRESS),
        (DAI, USDC),
    ];

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

    fn rule(name: &str, when: &str, fee: &str) -> RuleDefinition {
        RuleDefinition {
            name: name.to_owned(),
            when: when.to_owned(),
            fee: fee.to_owned(),
        }
    }

    fn list(tokens: &[Address]) -> Vec<String> {
        tokens.iter().map(Address::to_string).collect()
    }

    /// `prod_buckets` and a 2 bps default as rules, the way prod configures
    /// them: a list and a rule per bucket, with the cap of the legacy fees.
    fn prod_rules(max_partner_fee: Option<f64>) -> RuleSetDefinition {
        RuleSetDefinition {
            lists: BTreeMap::from([
                ("stablecoins".to_owned(), list(&[USDC, USDT])),
                (
                    "eth_correlated".to_owned(),
                    list(&[STETH, WETH, BUY_ETH_ADDRESS]),
                ),
            ]),
            rules: vec![
                rule(
                    "Same token (incl. wrapped native -> native): no volume fee",
                    "order.sellToken == order.buyToken || (order.sellToken in wrapped_native && \
                     order.buyToken == '0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee')",
                    "no_fee()",
                ),
                rule(
                    "Stablecoins: 0.3 bps",
                    "order.sellToken in stablecoins && order.buyToken in stablecoins",
                    "volume(0.3)",
                ),
                rule(
                    "ETH-correlated: 0.3 bps",
                    "order.sellToken in eth_correlated && order.buyToken in eth_correlated",
                    "volume(0.3)",
                ),
                rule("Default: 2 bps", "true", "volume(2)"),
            ],
            partner_fees: max_partner_fee.map(|cap| PartnerFees {
                max_volume: cap.try_into().unwrap(),
            }),
        }
    }

    /// The engine `new` loads: `definition` with the list `wrapped_native`,
    /// validated on the sample orders.
    fn engine_with(mut definition: RuleSetDefinition) -> FeeEngineVolumeFees {
        definition
            .lists
            .insert(WRAPPED_NATIVE.to_owned(), list(&[WETH]));
        let rules = RuleSet::new(definition).unwrap();
        rules.validate_on_sample_order().unwrap();
        FeeEngineVolumeFees { rules }
    }

    fn engine(max_partner_fee: Option<f64>) -> FeeEngineVolumeFees {
        engine_with(prod_rules(max_partner_fee))
    }

    /// The engine with only `rules`.
    fn engine_with_rules(rules: Vec<RuleDefinition>) -> FeeEngineVolumeFees {
        engine_with(RuleSetDefinition {
            rules,
            ..Default::default()
        })
    }

    /// The legacy volume fees `[volume-fee]` with `default` and `buckets` give.
    fn legacy(
        buckets: Vec<TokenBucketFeeOverride>,
        default: f64,
        same_token_fee: bool,
        max_partner_fee: Option<f64>,
    ) -> LegacyVolumeFees {
        LegacyVolumeFees {
            config: Some(VolumeFeeConfig {
                factor: Some(factor(default)),
                effective_from_timestamp: None,
            }),
            policy: Some(Arc::new(VolumeFeePolicy::new(
                buckets,
                Some(factor(default)),
                same_token_fee,
                WETH,
            ))),
            max_partner_fee: max_partner_fee.map(factor),
        }
    }

    fn compared(default: f64) -> ComparedVolumeFees {
        ComparedVolumeFees::new(legacy(prod_buckets(), default, false, None), engine(None))
    }

    fn app_data(document: &str) -> ValidatedAppData {
        ::app_data::Validator::new(8192)
            .validate(document.as_bytes())
            .unwrap()
    }

    /// The order a quote for `sell -> buy` describes.
    fn order(sell_token: Address, buy_token: Address) -> Order {
        let quote = quote();
        Order {
            data: OrderData {
                sell_token,
                buy_token,
                sell_amount: quote.sell_amount,
                buy_amount: quote.buy_amount,
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn quote() -> Quote {
        Quote::new(
            None,
            QuoteData {
                quoted_sell_amount: U256::from(1000),
                quoted_buy_amount: U256::from(2000),
                kind: OrderKind::Sell,
                ..Default::default()
            },
        )
    }

    fn compare_quote(compared: &ComparedVolumeFees, (sell, buy): (Address, Address)) -> Outcome {
        compared
            .quote_outcome(&order(sell, buy), &quote(), &app_data("{}"), None)
            .1
    }

    #[test]
    fn agrees_with_the_volume_fee_policy() {
        let compared = compared(0.0002);
        for (sell, buy) in PAIRS {
            assert_eq!(
                compare_quote(&compared, (sell, buy)),
                Outcome::Match,
                "{sell:?} -> {buy:?}"
            );
            let (evaluation, _) =
                compared
                    .engine
                    .quote_fees(&order(sell, buy), &app_data("{}"), None);
            assert!(evaluation.skipped.is_empty(), "{:?}", evaluation.skipped);
        }
    }

    #[test]
    fn reports_a_mismatch() {
        // A different default factor.
        assert_eq!(
            compare_quote(&compared(0.0003), (USDC, WETH)),
            Outcome::Mismatch
        );
        // A volume fee on same token trades.
        let compared =
            ComparedVolumeFees::new(legacy(prod_buckets(), 0.0002, true, None), engine(None));
        assert_eq!(compare_quote(&compared, (USDC, USDC)), Outcome::Mismatch);
    }

    #[test]
    fn a_zero_factor_is_a_quote_mismatch() {
        // A 0 bps bucket shows as `protocolFeeBps: "0"`; `no_fee()` shows none.
        let zero_bucket = TokenBucketFeeOverride {
            factor: factor(0.),
            ..bucket(&[USDC, USDT])
        };
        let compared = ComparedVolumeFees::new(
            legacy(vec![zero_bucket], 0.0002, false, None),
            engine_with_rules(vec![rule("x", "true", "no_fee()")]),
        );
        assert_eq!(compare_quote(&compared, (USDC, USDT)), Outcome::Mismatch);
    }

    #[test]
    fn a_quote_with_several_volume_fees_is_a_mismatch() {
        let compared = ComparedVolumeFees::new(
            legacy(vec![], 0.0002, false, None),
            engine_with_rules(vec![rule("x", "true", "[volume(2), volume(2)]")]),
        );
        assert_eq!(
            compared.quote_outcome(&order(USDC, WETH), &quote(), &app_data("{}"), None),
            (Some(factor(0.0002)), Outcome::Mismatch)
        );
    }

    #[test]
    fn a_skipped_rule_is_a_mismatch() {
        // The first rule fails on the quote's order (sell amount 1000) only;
        // the default then charges the legacy 2 bps.
        let rules = vec![
            rule(
                "x",
                "order.sellAmount == 1000.0 ? order.missing == 1 : false",
                "volume(1)",
            ),
            rule("y", "true", "volume(2)"),
        ];
        let compared = ComparedVolumeFees::new(
            legacy(vec![], 0.0002, false, None),
            engine_with_rules(rules),
        );
        assert_eq!(
            compared.quote_outcome(&order(USDC, WETH), &quote(), &app_data("{}"), None),
            (Some(factor(0.0002)), Outcome::Mismatch)
        );
    }

    #[test]
    fn serves_the_engine_fees_on_a_match_else_the_legacy_ones() {
        // The rules charge 2 bps by default.
        let serve = |fees: &dyn OrderbookFees| {
            fees.quote_volume_fee(&order(USDC, WETH), &quote(), &app_data("{}"), None)
        };
        let legacy = |default| legacy(prod_buckets(), default, false, None);
        // 2 bps on the grid, so a match; the factor served is the engine's.
        let noisy = 0.000_200_000_1;
        let without_rules = volume_fees(None, legacy(noisy), WETH).unwrap();
        assert_eq!(serve(&*without_rules), Some(factor(noisy)));
        let compared = |default| ComparedVolumeFees::new(legacy(default), engine(None));
        assert_eq!(serve(&compared(noisy)), Some(factor(0.0002)));
        assert_eq!(serve(&compared(0.0003)), Some(factor(0.0003)));
    }

    #[test]
    fn rules_must_load() {
        let broken =
            toml::from_str("[[rule]]\nname = 'x'\nwhen = 'true'\nfee = 'volume('\n").unwrap();
        assert!(volume_fees(Some(&broken), LegacyVolumeFees::default(), WETH).is_err());
        let missing_fee = toml::from_str("[[rule]]\nname = 'x'\nwhen = 'true'\n").unwrap();
        let error = volume_fees(Some(&missing_fee), LegacyVolumeFees::default(), WETH)
            .err()
            .unwrap();
        assert!(
            format!("{error:#}").contains("rule number 1 is malformed: missing field `fee`"),
            "{error:#}"
        );
    }

    #[test]
    fn the_section_may_not_set_wrapped_native() {
        // The rule loads only with the list the orderbook adds.
        let mut config: FeeEngineConfig = toml::from_str(
            "[[rule]]\nname = 'x'\nwhen = 'order.sellToken in wrapped_native'\nfee = 'volume(1)'\n",
        )
        .unwrap();
        assert!(FeeEngineVolumeFees::new(&config, WETH).is_ok());
        config.lists = Some(toml::from_str("wrapped_native = []").unwrap());
        assert!(FeeEngineVolumeFees::new(&config, WETH).is_err());
    }

    #[test]
    fn quote_keeps_every_volume_fee_of_the_match() {
        let engine = engine_with_rules(vec![rule("x", "true", "[volume(10), volume(10)]")]);
        assert_eq!(
            engine
                .quote_fees(&order(USDC, WETH), &app_data("{}"), None)
                .1,
            [factor(0.001), factor(0.001)]
        );
    }

    fn compare_fast_path(max_partner_fee: Option<f64>, document: &str) -> Outcome {
        let compared = ComparedVolumeFees::new(
            legacy(prod_buckets(), 0.0002, false, max_partner_fee),
            engine(max_partner_fee),
        );
        compared
            .fast_path_outcome(&order(USDC, WETH), &quote(), &app_data(document))
            .1
    }

    const VOLUME_PARTNER_FEES: &str = r#"{"metadata": {"partnerFee": [
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 60},
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 70}
    ]}}"#;

    #[test]
    fn quotes_leave_partner_fees_out() {
        let order = order(USDC, WETH);
        let app_data = app_data(VOLUME_PARTNER_FEES);
        let legacy = legacy(prod_buckets(), 0.0002, false, Some(0.01));
        assert_eq!(
            legacy.quote_volume_fee(&order, &quote(), &app_data, None),
            Some(factor(0.0002))
        );
        assert_eq!(
            engine(Some(0.01)).quote_fees(&order, &app_data, None).1,
            [factor(0.0002)]
        );
    }

    #[test]
    fn fast_path_agrees_on_capped_partner_volume_fees() {
        assert_eq!(compare_fast_path(Some(0.01), "{}"), Outcome::Match);
        assert_eq!(
            compare_fast_path(Some(0.01), VOLUME_PARTNER_FEES),
            Outcome::Match
        );
        // Without a cap the orderbook counts no partner fee.
        assert_eq!(compare_fast_path(None, VOLUME_PARTNER_FEES), Outcome::Match);
        // At the quoted price a partner surplus fee charges nothing, so it
        // leaves the whole cap to the volume fee after it.
        let surplus_first = r#"{"metadata": {"partnerFee": [
            {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "surplusBps": 50, "maxVolumeBps": 80},
            {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 50}
        ]}}"#;
        assert_eq!(compare_fast_path(Some(0.01), surplus_first), Outcome::Match);
    }
}

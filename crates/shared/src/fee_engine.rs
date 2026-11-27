//! The rules of `[fee-engine]` and an order's app data as the input of the fee
//! engine, for the services that host it, and the orderbook's volume fees.

use {
    crate::{
        order_quoting::Quote,
        order_validation::{Amounts, is_order_outside_market_price},
    },
    alloy::primitives::Address,
    anyhow::Context,
    app_data::{FeePolicy, ProtocolAppData, ValidatedAppData},
    chrono::Utc,
    configs::{fee_engine::FeeEngineConfig, fee_factor::FeeFactor},
    fee_engine::{Evaluation, Fee, PartnerFee, RuleSet, RulesToml},
    model::order::Order,
};

/// The list every host sets to the chain's wrapped native token.
pub const WRAPPED_NATIVE: &str = "wrapped_native";

/// Loads the rules of `[fee-engine]`, with the list `wrapped_native` and the
/// `host_lists` added, which the section may not set.
pub fn load_host_rules(
    config: &FeeEngineConfig,
    native_token: Address,
    host_lists: &[(&str, &[Address])],
) -> anyhow::Result<RuleSet> {
    let mut tables = RulesToml {
        lists: config.lists.clone(),
        rules: config.rules.clone(),
        partner_fees: config.partner_fees.clone(),
    };
    tables.add_host_list(WRAPPED_NATIVE, [native_token])?;
    for (name, values) in host_lists {
        tables.add_host_list(name, *values)?;
    }
    let rules = fee_engine::load_rules_toml(tables).context("loading the fee engine rules")?;
    rules.reset_skipped_rule_counters();
    let names: Vec<_> = rules.rules().iter().map(|rule| rule.name()).collect();
    tracing::info!(rules = ?names, "fee engine rules loaded");
    Ok(rules)
}

/// The app data of an order as the rules see it.
pub fn app_data(protocol: &ProtocolAppData, app_code: Option<String>) -> fee_engine::AppData {
    fee_engine::AppData {
        app_code,
        flashloan: protocol.flashloan.is_some(),
    }
}

/// The partner fees of the app data, for the engine to cap.
pub fn partner_fees(protocol: &ProtocolAppData) -> Vec<PartnerFee> {
    protocol
        .partner_fee
        .iter()
        .map(|fee| match fee.policy {
            FeePolicy::Volume { bps } => PartnerFee::Volume { bps },
            FeePolicy::Surplus {
                bps,
                max_volume_bps,
            } => PartnerFee::Surplus {
                bps,
                max_volume_bps,
            },
            FeePolicy::PriceImprovement {
                bps,
                max_volume_bps,
            } => PartnerFee::PriceImprovement {
                bps,
                max_volume_bps,
            },
        })
        .collect()
}

/// The partner fees a fast-path order pays. It settles at the quote, so only
/// partner volume fees charge anything; the others must not take the cap's
/// room.
pub fn fast_path_partner_fees(protocol: &ProtocolAppData) -> Vec<PartnerFee> {
    partner_fees(protocol)
        .into_iter()
        .filter(|fee| matches!(fee, PartnerFee::Volume { .. }))
        .collect()
}

/// The volume fees among the fees the rules charge. The orderbook needs no
/// other: surplus and price improvement fees take nothing at the quoted price.
pub struct FeeEngineVolumeFees {
    rules: RuleSet,
    canonical_batches: bool,
}

impl FeeEngineVolumeFees {
    /// Loads the rules of `[fee-engine]`, with the chain's wrapped native
    /// token added as the list `wrapped_native`, which the section may not set.
    /// `canonical_batches` tells whether the orderbook runs canonical batches.
    pub fn new(
        config: &FeeEngineConfig,
        native_token: Address,
        canonical_batches: bool,
    ) -> anyhow::Result<Self> {
        let rules = load_host_rules(config, native_token, &[])?;
        Ok(Self {
            rules,
            canonical_batches,
        })
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
                app_data: &self::app_data(&app_data.protocol, app_code),
                partner_fees,
                in_market,
                canonical_batches: self.canonical_batches,
            });
        evaluation.warn_skipped();
        evaluation
    }

    /// The protocol volume fee a quote gets; `order` is the one the quote
    /// describes, before the fee. A quote shows one volume fee: the first of
    /// the rule that matched.
    pub fn quote_volume_fee(
        &self,
        order: &Order,
        app_data: &ValidatedAppData,
        app_code: Option<&str>,
    ) -> Option<FeeFactor> {
        // The quote leaves partner fees out and sets the market price.
        let app_code = app_code.map(str::to_owned);
        let evaluation = self.evaluate(order, app_data, app_code, &[], true);
        protocol_volume_fees(&evaluation).first().copied()
    }

    /// The protocol, then the capped partner, volume fees a fast-path order
    /// must leave room for, in the order the autopilot compounds them.
    pub fn fast_path_volume_fees(
        &self,
        order: &Order,
        quote: &Quote,
        app_data: &ValidatedAppData,
    ) -> Vec<FeeFactor> {
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
        let partner_fees = fast_path_partner_fees(&app_data.protocol);
        // Placement reads the app code nowhere else, so this parse is its only
        // one.
        let app_code = ::app_data::app_code(app_data.document.as_bytes());
        let evaluation = self.evaluate(order, app_data, app_code, &partner_fees, in_market);
        let mut fees = protocol_volume_fees(&evaluation);
        fees.extend(volume_factors(&evaluation.partner_fees));
        fees
    }
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::order_quoting::QuoteData,
        alloy::primitives::{U256, address},
        model::order::{BUY_ETH_ADDRESS, OrderData, OrderKind},
    };

    const WETH: Address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    const STETH: Address = address!("ae7ab96520de3a18e5e111b5eaab095312d7fe84");
    const USDC: Address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    const USDT: Address = address!("dac17f958d2ee523a2206206994597c13d831ec7");

    /// The rules the way prod configures them: a list and a rule per bucket.
    const RULES: &str = r#"
[lists]
stablecoins = ["0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", "0xdac17f958d2ee523a2206206994597c13d831ec7"]
eth_correlated = ["0xae7ab96520de3a18e5e111b5eaab095312d7fe84", "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"]

[[rule]]
name = "Same token (incl. wrapped native -> native): no volume fee"
when = 'order.sellToken == order.buyToken || (order.sellToken in wrapped_native && order.buyToken == "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")'
fee = "no_fee()"

[[rule]]
name = "Stablecoins: 0.3 bps"
when = "order.sellToken in stablecoins && order.buyToken in stablecoins"
fee = "volume(0.3)"

[[rule]]
name = "ETH-correlated: 0.3 bps"
when = "order.sellToken in eth_correlated && order.buyToken in eth_correlated"
fee = "volume(0.3)"

[[rule]]
name = "Default: 2 bps"
when = "true"
fee = "volume(2)"
"#;

    fn factor(value: f64) -> FeeFactor {
        FeeFactor::try_from(value).unwrap()
    }

    /// The engine with `rules` and, with `max_partner_fee_bps`, the cap.
    fn engine_with(rules: &str, max_partner_fee_bps: Option<u32>) -> FeeEngineVolumeFees {
        let cap = max_partner_fee_bps
            .map(|bps| format!("[partner-fees]\nmax-volume-bps = {bps}\n"))
            .unwrap_or_default();
        let config = toml::from_str(&format!("{cap}{rules}")).unwrap();
        FeeEngineVolumeFees::new(&config, WETH, false).unwrap()
    }

    fn engine(max_partner_fee_bps: Option<u32>) -> FeeEngineVolumeFees {
        engine_with(RULES, max_partner_fee_bps)
    }

    /// The engine with one rule that charges `fee` on every order.
    fn engine_charging(fee: &str) -> FeeEngineVolumeFees {
        let rule = format!("[[rule]]\nname = 'x'\nwhen = 'true'\nfee = '{fee}'\n");
        engine_with(&rule, None)
    }

    fn validated(document: &str) -> ValidatedAppData {
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

    const VOLUME_PARTNER_FEES: &str = r#"{"metadata": {"partnerFee": [
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 60},
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 70}
    ]}}"#;

    #[test]
    fn the_first_rule_that_matches_decides_the_quote_volume_fee() {
        let engine = engine(None);
        let fee = |sell, buy| engine.quote_volume_fee(&order(sell, buy), &validated("{}"), None);
        let bucket = Some(factor(0.00003));
        assert_eq!(fee(USDC, USDT), bucket);
        assert_eq!(fee(WETH, STETH), bucket);
        assert_eq!(fee(STETH, BUY_ETH_ADDRESS), bucket);
        assert_eq!(fee(BUY_ETH_ADDRESS, STETH), bucket);
        assert_eq!(fee(USDC, USDC), None);
        assert_eq!(fee(WETH, BUY_ETH_ADDRESS), None);
        assert_eq!(fee(USDC, WETH), Some(factor(0.0002)));
        assert_eq!(fee(USDC, BUY_ETH_ADDRESS), Some(factor(0.0002)));
    }

    #[test]
    fn a_zero_volume_fee_is_a_quote_volume_fee() {
        // It shows as `protocolFeeBps: "0"`; `no_fee()` shows none.
        let fee = |engine: FeeEngineVolumeFees| {
            engine.quote_volume_fee(&order(USDC, USDT), &validated("{}"), None)
        };
        assert_eq!(fee(engine_charging("volume(0)")), Some(factor(0.)));
        assert_eq!(fee(engine_charging("no_fee()")), None);
    }

    #[test]
    fn quote_shows_the_first_volume_fee_of_the_match() {
        let engine = engine_charging("[surplus(50, 98), volume(10), volume(20)]");
        assert_eq!(
            engine.quote_volume_fee(&order(USDC, WETH), &validated("{}"), None),
            Some(factor(0.001))
        );
    }

    #[test]
    fn a_skipped_rule_leaves_the_fee_to_the_next_match() {
        // The first rule fails on the quote's order (sell amount 1000) only.
        let rules = r#"
[[rule]]
name = "x"
when = "order.sellAmount == 1000.0 ? order.missing == 1 : false"
fee = "volume(1)"

[[rule]]
name = "y"
when = "true"
fee = "volume(2)"
"#;
        assert_eq!(
            engine_with(rules, None).quote_volume_fee(&order(USDC, WETH), &validated("{}"), None),
            Some(factor(0.0002))
        );
    }

    #[test]
    fn no_rules_charge_no_volume_fee() {
        let engine = FeeEngineVolumeFees::new(&FeeEngineConfig::default(), WETH, false).unwrap();
        let (order, app_data) = (order(USDC, WETH), validated(VOLUME_PARTNER_FEES));
        assert_eq!(engine.quote_volume_fee(&order, &app_data, None), None);
        assert_eq!(
            engine.fast_path_volume_fees(&order, &quote(), &app_data),
            []
        );
    }

    #[test]
    fn rules_must_load() {
        let broken =
            toml::from_str("[[rule]]\nname = 'x'\nwhen = 'true'\nfee = 'volume('\n").unwrap();
        assert!(FeeEngineVolumeFees::new(&broken, WETH, false).is_err());
        let missing_fee = toml::from_str("[[rule]]\nname = 'x'\nwhen = 'true'\n").unwrap();
        let error = FeeEngineVolumeFees::new(&missing_fee, WETH, false)
            .err()
            .unwrap();
        assert!(
            format!("{error:#}").contains("rule number 1 is malformed: missing field `fee`"),
            "{error:#}"
        );
    }

    #[test]
    fn the_section_may_not_set_wrapped_native() {
        // The rule loads only with the list the host adds.
        let mut config: FeeEngineConfig = toml::from_str(
            "[[rule]]\nname = 'x'\nwhen = 'order.sellToken in wrapped_native'\nfee = 'volume(1)'\n",
        )
        .unwrap();
        assert!(FeeEngineVolumeFees::new(&config, WETH, false).is_ok());
        config.lists = Some(toml::from_str("wrapped_native = []").unwrap());
        assert!(FeeEngineVolumeFees::new(&config, WETH, false).is_err());
    }

    #[test]
    fn wait_for_cow_quotes_skip_the_volume_fee_only_with_canonical_batches() {
        let config: FeeEngineConfig = toml::from_str(
            r#"
[[rule]]
name = "Wait for CoW: no volume fee"
when = "order.waitForCow"
fee = "surplus(50, 98)"

[[rule]]
name = "Default: 2 bps"
when = "true"
fee = "[surplus(50, 98), volume(2)]"
"#,
        )
        .unwrap();
        let fee = |wait_for_cow, canonical_batches| {
            let mut order = order(USDC, WETH);
            order.metadata.wait_for_cow = wait_for_cow;
            FeeEngineVolumeFees::new(&config, WETH, canonical_batches)
                .unwrap()
                .quote_volume_fee(&order, &validated("{}"), None)
        };
        assert_eq!(fee(true, true), None);
        assert_eq!(fee(true, false), Some(factor(0.0002)));
        assert_eq!(fee(false, true), Some(factor(0.0002)));
    }

    #[test]
    fn quotes_leave_partner_fees_out() {
        assert_eq!(
            engine(Some(100)).quote_volume_fee(
                &order(USDC, WETH),
                &validated(VOLUME_PARTNER_FEES),
                None
            ),
            Some(factor(0.0002))
        );
    }

    #[test]
    fn fast_path_fees_are_the_protocol_then_the_capped_partner_volume_fees() {
        let fees = |max_partner_fee_bps, document| {
            engine(max_partner_fee_bps).fast_path_volume_fees(
                &order(USDC, WETH),
                &quote(),
                &validated(document),
            )
        };
        let protocol = factor(0.0002);
        assert_eq!(fees(Some(100), "{}"), [protocol]);
        // What the 1% cap leaves after the 0.6%.
        let capped = factor(0.003976143141153082);
        assert_eq!(
            fees(Some(100), VOLUME_PARTNER_FEES),
            [protocol, factor(0.006), capped]
        );
        // Without a cap the engine counts no partner fee.
        assert_eq!(fees(None, VOLUME_PARTNER_FEES), [protocol]);
        // At the quoted price a partner surplus fee charges nothing, so it
        // leaves the whole cap to the volume fee after it.
        let surplus_first = r#"{"metadata": {"partnerFee": [
            {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "surplusBps": 50, "maxVolumeBps": 80},
            {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 50}
        ]}}"#;
        assert_eq!(fees(Some(100), surplus_first), [protocol, factor(0.005)]);
    }

    #[test]
    fn fast_path_keeps_only_partner_volume_fees() {
        let protocol = app_data::parse(
            br#"{"metadata": {"partnerFee": [
                {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "surplusBps": 50, "maxVolumeBps": 80},
                {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 50}
            ]}}"#,
        )
        .unwrap();
        assert_eq!(
            partner_fees(&protocol),
            [
                PartnerFee::Surplus {
                    bps: 50,
                    max_volume_bps: 80
                },
                PartnerFee::Volume { bps: 50 },
            ]
        );
        assert_eq!(
            fast_path_partner_fees(&protocol),
            [PartnerFee::Volume { bps: 50 }]
        );
    }
}

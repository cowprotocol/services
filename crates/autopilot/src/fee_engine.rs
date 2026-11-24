//! The autopilot's fee policies from the rules of `[fee-engine]`.

use {
    crate::{
        boundary,
        domain::{
            self,
            fee::{Origin, Policy, PolicyWithOrigin, Quote, reference_quote},
        },
    },
    alloy::primitives::Address,
    chrono::{DateTime, Utc},
    configs::{fee_engine::FeeEngineConfig, fee_factor::FeeFactor},
    fee_engine::{Evaluation, Fee, RuleSet},
    shared::fee_engine::load_host_rules,
};

/// The list the autopilot sets to the configured surplus capturing JIT order
/// owners.
const JIT_OWNERS: &str = "jit_owners";

/// The rules of `[fee-engine]` as a source of fee policies. It evaluates them
/// for each order it is given: the fees of the first rule that matches, then
/// the order's capped partner fees.
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
        let rules = load_host_rules(
            config,
            native_token,
            &[(JIT_OWNERS, surplus_capturing_jit_order_owners)],
        )?;
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

    /// The fee policies of an order in an auction, protocol then partner.
    pub fn auction_fees(
        &self,
        order: &boundary::Order,
        quote: Option<&domain::Quote>,
        now: DateTime<Utc>,
    ) -> Vec<PolicyWithOrigin> {
        fee_engine::time_evaluation(|| {
            let quote = reference_quote(order, quote);
            let in_market = !boundary::is_order_outside_market_price(
                &order.into(),
                &(&quote).into(),
                order.data.kind,
            );
            policies(&self.evaluate(order, in_market, now), &quote)
        })
    }
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
    let convert = |factor: fee_engine::FeeFactor| FeeFactor::new(factor.to_f64());
    match fee {
        Fee::Volume { factor } => Policy::Volume {
            factor: convert(factor),
        },
        Fee::Surplus {
            factor,
            max_volume_factor,
        } => Policy::Surplus {
            factor: convert(factor),
            max_volume_factor: convert(max_volume_factor),
        },
        Fee::PriceImprovement {
            factor,
            max_volume_factor,
        } => Policy::PriceImprovement {
            factor: convert(factor),
            max_volume_factor: convert(max_volume_factor),
            quote: Quote::from_domain(quote),
        },
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{U256, address},
        model::order::{BUY_ETH_ADDRESS, OrderData, OrderKind, OrderMetadata},
    };

    const WETH: Address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    const STETH: Address = address!("ae7ab96520de3a18e5e111b5eaab095312d7fe84");
    const USDC: Address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    const USDT: Address = address!("dac17f958d2ee523a2206206994597c13d831ec7");
    const JIT_OWNER: Address = address!("1111111111111111111111111111111111111111");

    /// The rules the way prod configures them: a list and a rule per bucket.
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

    fn engine() -> FeeEngineFees {
        FeeEngineFees::new(&toml::from_str(RULES).unwrap(), WETH, &[JIT_OWNER]).unwrap()
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

    fn surplus() -> PolicyWithOrigin {
        PolicyWithOrigin {
            policy: Policy::Surplus {
                factor: factor(0.5),
                max_volume_factor: factor(0.0098),
            },
            origin: Origin::Protocol,
        }
    }

    fn price_improvement(quote: &domain::Quote) -> PolicyWithOrigin {
        PolicyWithOrigin {
            policy: Policy::PriceImprovement {
                factor: factor(0.5),
                max_volume_factor: factor(0.0098),
                quote: Quote::from_domain(quote),
            },
            origin: Origin::Protocol,
        }
    }

    const VOLUME_PARTNER_FEES: &str = r#"{"appCode": "CoW Swap", "metadata": {"partnerFee": [
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 400},
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 300}
    ]}}"#;

    #[test]
    fn the_quote_decides_between_price_improvement_and_surplus() {
        let engine = engine();
        let order = order(USDC, WETH, None);
        let fees = |quote: Option<&domain::Quote>| engine.auction_fees(&order, quote, Utc::now());
        // The order is inside the market price of the first quote only.
        let (inside, outside) = (quote(2000), quote(500));
        assert_eq!(
            fees(Some(&inside)),
            [price_improvement(&inside), volume(0.0002, Origin::Protocol)]
        );
        assert_eq!(
            fees(Some(&outside)),
            [surplus(), volume(0.0002, Origin::Protocol)]
        );
        assert_eq!(fees(None), [surplus(), volume(0.0002, Origin::Protocol)]);
    }

    #[test]
    fn the_first_rule_that_matches_decides_the_volume_fee() {
        let engine = engine();
        let fees = |sell, buy| engine.auction_fees(&order(sell, buy, None), None, Utc::now());
        let bucket = [surplus(), volume(0.00003, Origin::Protocol)];
        assert_eq!(fees(USDC, USDT), bucket);
        assert_eq!(fees(WETH, STETH), bucket);
        assert_eq!(fees(STETH, BUY_ETH_ADDRESS), bucket);
        assert_eq!(fees(USDC, USDC), [surplus()]);
        assert_eq!(fees(WETH, BUY_ETH_ADDRESS), [surplus()]);
        assert_eq!(
            fees(USDC, WETH),
            [surplus(), volume(0.0002, Origin::Protocol)]
        );
    }

    #[test]
    fn auction_fees_are_the_protocol_fees_then_the_capped_partner_fees() {
        let order = order(USDC, WETH, Some(VOLUME_PARTNER_FEES));
        let quote = quote(2000);
        assert_eq!(
            engine().auction_fees(&order, Some(&quote), Utc::now()),
            [
                price_improvement(&quote),
                volume(0.0002, Origin::Protocol),
                volume(0.04, Origin::Partner),
                // What the 5% cap leaves after the 4%.
                volume(0.009615384615384616, Origin::Partner),
            ]
        );
    }

    #[test]
    fn an_order_without_a_document_that_parses_has_no_partner_fee() {
        let engine = engine();
        for document in [None, Some("{}"), Some("not a document")] {
            assert_eq!(
                engine.auction_fees(&order(USDC, WETH, document), None, Utc::now()),
                [surplus(), volume(0.0002, Origin::Protocol)],
                "{document:?}"
            );
        }
    }

    #[test]
    fn a_skipped_rule_leaves_the_fees_to_the_next_match() {
        // The first rule fails on the order (sell amount 1000) only.
        let config = toml::from_str(
            r#"
[[rule]]
name = "x"
when = "order.sellAmount == 1000.0 ? order.missing == 1 : false"
fee = "volume(1)"

[[rule]]
name = "y"
when = "true"
fee = "volume(2)"
"#,
        )
        .unwrap();
        let engine = FeeEngineFees::new(&config, WETH, &[]).unwrap();
        assert_eq!(
            engine.auction_fees(&order(USDC, WETH, None), None, Utc::now()),
            [volume(0.0002, Origin::Protocol)]
        );
    }

    #[test]
    fn no_rules_charge_no_fee() {
        let engine = FeeEngineFees::new(&FeeEngineConfig::default(), WETH, &[]).unwrap();
        let order = order(USDC, WETH, Some(VOLUME_PARTNER_FEES));
        assert_eq!(engine.auction_fees(&order, None, Utc::now()), []);
    }

    #[test]
    fn rules_must_load() {
        let broken =
            toml::from_str("[[rule]]\nname = 'x'\nwhen = 'true'\nfee = 'volume('\n").unwrap();
        assert!(FeeEngineFees::new(&broken, WETH, &[]).is_err());
    }

    #[test]
    fn jit_owners_pay_only_partner_fees() {
        let mut order = order(USDC, WETH, Some(VOLUME_PARTNER_FEES));
        order.metadata.owner = JIT_OWNER;
        assert_eq!(
            engine().auction_fees(&order, None, Utc::now()),
            [
                volume(0.04, Origin::Partner),
                volume(0.009615384615384616, Origin::Partner),
            ]
        );
    }
}

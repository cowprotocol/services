//! Runs the fee engine next to the orderbook's volume fees without acting on
//! them, and reports every quote and fast-path order where the two disagree.

use {
    alloy::primitives::Address,
    anyhow::Context,
    app_data::{ExecutionMode, FeePolicy, ValidatedAppData},
    chrono::Utc,
    configs::fee_factor::FeeFactor,
    fee_engine::{Evaluation, Fee, PartnerFee, RuleSet},
    model::{
        order::{BUY_ETH_ADDRESS, Order, OrderData, OrderMetadata, OrderUid},
        quote::{OrderQuoteRequest, QuoteId},
        signature::Signature,
    },
    shared::{
        arguments::TokenBucketFeeOverride,
        order_quoting::Quote,
        order_validation::{Amounts, FastPathFeeShadow, is_order_outside_market_price},
    },
    std::collections::BTreeMap,
};

/// The orderbook's volume fee as rules, with the factors prod configures. The
/// host adds `[partner_fees]` and `[lists]` from the orderbook's config; see
/// [`host_tables`].
const RULES: &str = r#"
[[rule]]
name = "Same token (incl. wrapped native -> native): no volume fee"
when = 'order.sellToken == order.buyToken || (order.sellToken in wrapped_native && order.buyToken == "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")'
fee = "no_fee()"

[[rule]]
name = "Volume fee bucket: 0.3 bps"
when = 'buckets.exists(bucket, bucket + order.sellToken in bucket_tokens && bucket + (order.buyToken == "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee" ? wrapped_native[0] : order.buyToken) in bucket_tokens)'
fee = "volume(0.3)"

[[rule]]
name = "Default: 2 bps"
when = "true"
fee = "volume(2)"
"#;

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "orderbook")]
struct Metrics {
    /// Volume fee decisions the fee engine checked, by whether it agreed.
    #[metric(labels("check", "result"))]
    fee_engine_shadow: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(observe::metrics::get_storage_registry())
            .expect("unexpected error getting metrics instance")
    }
}

/// Which volume fee decision of the orderbook the engine checked.
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

/// Whether the fee engine charged the volume fees the orderbook charged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Match,
    Mismatch,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Mismatch => "mismatch",
        }
    }
}

pub struct FeeEngineShadow {
    rules: RuleSet,
}

impl FeeEngineShadow {
    /// Loads [`RULES`] with the buckets, native token and partner fee cap of
    /// the orderbook's config.
    pub fn new(
        buckets: &[TokenBucketFeeOverride],
        native_token: Address,
        max_partner_fee: Option<FeeFactor>,
    ) -> anyhow::Result<Self> {
        let tables = host_tables(buckets, native_token, max_partner_fee)?;
        let rules = fee_engine::load_rules(&format!("{tables}\n{RULES}"))
            .context("loading the fee engine shadow rules")?;
        let names: Vec<_> = rules.rules().iter().map(|rule| rule.name()).collect();
        tracing::info!(rules = ?names, "fee engine shadow enabled");
        let metrics = Metrics::get();
        for check in [Check::Quote, Check::FastPath] {
            for outcome in [Outcome::Match, Outcome::Mismatch] {
                metrics
                    .fee_engine_shadow
                    .with_label_values(&[check.as_str(), outcome.as_str()])
                    .reset();
            }
        }
        Ok(Self { rules })
    }

    /// Runs the rules on the order `quote` describes and compares their
    /// volume fee with `volume_fee`, the one the quote got.
    pub fn compare_quote(
        &self,
        request: &OrderQuoteRequest,
        quote: &Quote,
        valid_to: u32,
        app_data: &ValidatedAppData,
        app_code: Option<String>,
        volume_fee: Option<FeeFactor>,
    ) {
        self.quote_outcome(request, quote, valid_to, app_data, app_code, volume_fee);
    }

    fn quote_outcome(
        &self,
        request: &OrderQuoteRequest,
        quote: &Quote,
        valid_to: u32,
        app_data: &ValidatedAppData,
        app_code: Option<String>,
        volume_fee: Option<FeeFactor>,
    ) -> Outcome {
        let order = quote_order(request, quote, valid_to, app_data);
        let evaluation = self
            .rules
            .evaluator(Utc::now())
            .evaluate(fee_engine::Order {
                order: &order,
                app_data: &engine_app_data(app_data, app_code),
                // The quote's volume fee leaves partner fees out.
                partner_fees: &[],
                // The quote sets the market price.
                in_market: true,
            });
        let engine = protocol_volume_fees(&evaluation);
        report(
            Check::Quote,
            quote.id,
            None,
            &order,
            &evaluation,
            &engine,
            volume_fee.as_slice(),
        )
    }

    fn fast_path_outcome(
        &self,
        order: &Order,
        quote: &Quote,
        app_data: &ValidatedAppData,
        volume_fees: &[FeeFactor],
    ) -> Outcome {
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
        // A fast-path order settles at the quote, so only partner volume
        // fees charge anything; the others must not take the cap's room.
        let partner_fees: Vec<_> = app_data
            .protocol
            .partner_fee
            .iter()
            .filter_map(|fee| match fee.policy {
                FeePolicy::Volume { bps } => Some(PartnerFee::Volume { bps }),
                FeePolicy::Surplus { .. } | FeePolicy::PriceImprovement { .. } => None,
            })
            .collect();
        let evaluation = self
            .rules
            .evaluator(Utc::now())
            .evaluate(fee_engine::Order {
                order,
                app_data: &engine_app_data(
                    app_data,
                    app_data::app_code(app_data.document.as_bytes()),
                ),
                partner_fees: &partner_fees,
                in_market,
            });
        let mut engine = protocol_volume_fees(&evaluation);
        engine.extend(volume_factors(&evaluation.partner_fees));
        report(
            Check::FastPath,
            quote.id,
            Some(order.metadata.uid),
            order,
            &evaluation,
            &engine,
            volume_fees,
        )
    }
}

impl FastPathFeeShadow for FeeEngineShadow {
    fn compare_fast_path(
        &self,
        order: &Order,
        quote: &Quote,
        app_data: &ValidatedAppData,
        volume_fees: &[FeeFactor],
    ) {
        self.fast_path_outcome(order, quote, app_data, volume_fees);
    }
}

/// Logs the rules the engine skipped, and the volume fees if they disagree.
fn report(
    check: Check,
    quote_id: Option<QuoteId>,
    order_uid: Option<OrderUid>,
    order: &Order,
    evaluation: &Evaluation,
    engine: &[FeeFactor],
    orderbook: &[FeeFactor],
) -> Outcome {
    for skipped in &evaluation.skipped {
        tracing::warn!(check = check.as_str(), %skipped, "fee engine skipped a rule");
    }
    let outcome = if grid(engine).eq(grid(orderbook)) {
        Outcome::Match
    } else {
        let matched = evaluation.matched.first();
        tracing::warn!(
            check = check.as_str(),
            ?quote_id,
            ?order_uid,
            expected_bps = ?bps(orderbook),
            got_bps = ?bps(engine),
            rule = ?matched.map(|matched| matched.rule.name()),
            rule_fees = ?matched.map(|matched| &matched.fees),
            sell_token = ?order.data.sell_token,
            buy_token = ?order.data.buy_token,
            kind = ?order.data.kind,
            owner = ?order.metadata.owner,
            "fee engine disagrees with the orderbook's volume fees"
        );
        Outcome::Mismatch
    };
    Metrics::get()
        .fee_engine_shadow
        .with_label_values(&[check.as_str(), outcome.as_str()])
        .inc();
    outcome
}

/// `[partner_fees]` and `[lists]` for [`RULES`]: `wrapped_native`, and the
/// buckets as `buckets` (`"<i>:"`) and `bucket_tokens` (`"<i>:<token>"`). A
/// bucket holds the wrapped native token when it holds native ETH, as in
/// [`shared::fee::VolumeFeePolicy`].
fn host_tables(
    buckets: &[TokenBucketFeeOverride],
    native_token: Address,
    max_partner_fee: Option<FeeFactor>,
) -> anyhow::Result<String> {
    #[derive(serde::Serialize)]
    struct Tables {
        partner_fees: PartnerFees,
        lists: BTreeMap<&'static str, Vec<String>>,
    }
    #[derive(serde::Serialize)]
    struct PartnerFees {
        max_volume: f64,
    }

    let mut prefixes = Vec::new();
    let mut tokens = Vec::new();
    for (i, bucket) in buckets.iter().enumerate() {
        let prefix = format!("{i}:");
        let native = bucket
            .tokens
            .contains(&BUY_ETH_ADDRESS)
            .then_some(native_token);
        for token in bucket.tokens.iter().copied().chain(native) {
            // Lowercase here: the engine lowercases only plain hex members.
            tokens.push(format!("{prefix}{}", const_hex::encode_prefixed(token)));
        }
        prefixes.push(prefix);
    }
    let tables = Tables {
        // Without a cap, the orderbook counts no partner fee.
        partner_fees: PartnerFees {
            max_volume: max_partner_fee.map_or(0., |cap| {
                cap.to_high_precision() as f64 / (FeeFactor::HIGH_PRECISION_SCALE / 100) as f64
            }),
        },
        lists: BTreeMap::from([
            ("wrapped_native", vec![native_token.to_string()]),
            ("buckets", prefixes),
            ("bucket_tokens", tokens),
        ]),
    };
    Ok(toml::to_string(&tables)?)
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
            Fee::Volume { factor } => Some(*factor),
            Fee::Surplus { .. } | Fee::PriceImprovement { .. } => None,
        })
        .collect()
}

/// Factors in basis points, as `protocolFeeBps` shows them.
fn bps(factors: &[FeeFactor]) -> Vec<String> {
    factors.iter().map(FeeFactor::to_bps_str).collect()
}

/// Factors on the grid the fee math uses, so float noise is not a mismatch.
/// A zero factor charges nothing, like no factor.
fn grid(factors: &[FeeFactor]) -> impl Iterator<Item = u64> {
    factors
        .iter()
        .map(|factor| factor.to_high_precision())
        .filter(|&factor| factor != 0)
}

/// The order a user signs from the quote, before the volume fee.
fn quote_order(
    request: &OrderQuoteRequest,
    quote: &Quote,
    valid_to: u32,
    app_data: &ValidatedAppData,
) -> Order {
    let execution_mode = &app_data.protocol.execution_mode;
    Order {
        metadata: OrderMetadata {
            creation_date: Utc::now(),
            owner: request.from,
            fast_path: matches!(execution_mode, ExecutionMode::FastPath),
            valid_from: match execution_mode {
                ExecutionMode::ValidFrom(valid_from) => Some(*valid_from),
                ExecutionMode::RegularAuction | ExecutionMode::FastPath => None,
            },
            ..Default::default()
        },
        data: OrderData {
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            receiver: request.receiver,
            sell_amount: quote.sell_amount,
            buy_amount: quote.buy_amount,
            valid_to,
            app_data: app_data.hash,
            fee_amount: quote.fee_amount,
            kind: quote.data.kind,
            partially_fillable: false,
            sell_token_balance: request.sell_token_balance,
            buy_token_balance: request.buy_token_balance,
        },
        signature: Signature::default_with(request.signing_scheme.into()),
        interactions: Default::default(),
    }
}

fn engine_app_data(app_data: &ValidatedAppData, app_code: Option<String>) -> fee_engine::AppData {
    let protocol = &app_data.protocol;
    fee_engine::AppData {
        app_code,
        signer: protocol.signer,
        replaced_order: protocol
            .replaced_order
            .as_ref()
            .map(|replaced| replaced.uid.0),
        flashloan: protocol
            .flashloan
            .as_ref()
            .map(|flashloan| fee_engine::Flashloan {
                liquidity_provider: flashloan.liquidity_provider,
                protocol_adapter: flashloan.protocol_adapter,
                receiver: flashloan.receiver,
                token: flashloan.token,
                amount: flashloan.amount,
            }),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{U256, address},
        model::{
            order::OrderKind,
            quote::{OrderQuoteSide, SellAmount},
        },
        shared::{
            fee::{VolumeFeePolicy, capped_partner_volume_factors},
            order_quoting::QuoteData,
        },
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

    fn policy(buckets: Vec<TokenBucketFeeOverride>, default: f64) -> VolumeFeePolicy {
        VolumeFeePolicy::new(buckets, Some(factor(default)), false, WETH)
    }

    fn shadow(buckets: &[TokenBucketFeeOverride], max_partner_fee: Option<f64>) -> FeeEngineShadow {
        FeeEngineShadow::new(buckets, WETH, max_partner_fee.map(factor)).unwrap()
    }

    fn app_data(document: &str) -> ValidatedAppData {
        ::app_data::Validator::new(8192)
            .validate(document.as_bytes())
            .unwrap()
    }

    fn request(sell_token: Address, buy_token: Address) -> OrderQuoteRequest {
        OrderQuoteRequest {
            sell_token,
            buy_token,
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: U256::from(1000).try_into().unwrap(),
                },
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

    /// The quote check for `sell -> buy`, against the factor `policy` gives.
    fn compare_quote(
        shadow: &FeeEngineShadow,
        policy: &VolumeFeePolicy,
        (sell, buy): (Address, Address),
    ) -> Outcome {
        let volume_fee = policy.get_applicable_volume_fee_factor(buy, sell, None);
        shadow.quote_outcome(
            &request(sell, buy),
            &quote(),
            0,
            &app_data("{}"),
            None,
            volume_fee,
        )
    }

    #[test]
    fn agrees_with_the_volume_fee_policy() {
        for buckets in [
            prod_buckets(),
            vec![],
            vec![
                bucket(&[USDC]),
                bucket(&[USDT]),
                bucket(&[STETH, WETH, DAI, USDC]),
            ],
        ] {
            let shadow = shadow(&buckets, None);
            let policy = policy(buckets, 0.0002);
            for pair in PAIRS {
                assert_eq!(
                    compare_quote(&shadow, &policy, pair),
                    Outcome::Match,
                    "{pair:?}"
                );
            }
        }
    }

    #[test]
    fn rules_skip_nothing() {
        let shadow = shadow(&prod_buckets(), Some(0.01));
        let evaluator = shadow.rules.evaluator(Utc::now());
        for (sell, buy) in PAIRS {
            let order = quote_order(&request(sell, buy), &quote(), 0, &app_data("{}"));
            let evaluation = evaluator.evaluate(fee_engine::Order {
                order: &order,
                app_data: &Default::default(),
                partner_fees: &[],
                in_market: true,
            });
            assert!(evaluation.skipped.is_empty(), "{:?}", evaluation.skipped);
        }
    }

    #[test]
    fn reports_a_different_default_factor() {
        let shadow = shadow(&prod_buckets(), None);
        let policy = policy(prod_buckets(), 0.0003);
        assert_eq!(
            compare_quote(&shadow, &policy, (USDC, WETH)),
            Outcome::Mismatch
        );
    }

    #[test]
    fn reports_a_volume_fee_on_same_token_trades() {
        let shadow = shadow(&prod_buckets(), None);
        let policy = VolumeFeePolicy::new(prod_buckets(), Some(factor(0.0002)), true, WETH);
        assert_eq!(
            compare_quote(&shadow, &policy, (USDC, USDC)),
            Outcome::Mismatch
        );
    }

    #[test]
    fn zero_factor_is_no_factor() {
        assert!(grid(&[factor(0.)]).eq(grid(&[])));
        assert!(!grid(&[factor(0.0002)]).eq(grid(&[])));
    }

    fn compare_fast_path(max_partner_fee: Option<f64>, document: &str) -> Outcome {
        let shadow = shadow(&prod_buckets(), max_partner_fee);
        let app_data = app_data(document);
        let quote = quote();
        let order = Order {
            data: OrderData {
                sell_token: USDC,
                buy_token: WETH,
                sell_amount: quote.sell_amount,
                buy_amount: quote.buy_amount,
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };
        // What `OrderValidator::fast_path_volume_factors` computes.
        let protocol =
            policy(prod_buckets(), 0.0002).get_applicable_volume_fee_factor(WETH, USDC, None);
        let partners = max_partner_fee
            .map(|cap| capped_partner_volume_factors(&app_data.protocol, factor(cap)))
            .unwrap_or_default();
        let volume_fees: Vec<_> = protocol.into_iter().chain(partners).collect();
        shadow.fast_path_outcome(&order, &quote, &app_data, &volume_fees)
    }

    const VOLUME_PARTNER_FEES: &str = r#"{"metadata": {"partnerFee": [
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 60},
        {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 70}
    ]}}"#;

    #[test]
    fn fast_path_agrees_on_capped_partner_volume_fees() {
        assert_eq!(compare_fast_path(Some(0.01), "{}"), Outcome::Match);
        assert_eq!(
            compare_fast_path(Some(0.01), VOLUME_PARTNER_FEES),
            Outcome::Match
        );
        // Without a cap the orderbook counts no partner fee.
        assert_eq!(compare_fast_path(None, VOLUME_PARTNER_FEES), Outcome::Match);
    }

    #[test]
    fn fast_path_ignores_partner_surplus_fees() {
        // At the quoted price a partner surplus fee charges nothing, so it
        // leaves the whole cap to the volume fee after it.
        let document = r#"{"metadata": {"partnerFee": [
            {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "surplusBps": 50, "maxVolumeBps": 80},
            {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 50}
        ]}}"#;
        assert_eq!(compare_fast_path(Some(0.01), document), Outcome::Match);
    }
}

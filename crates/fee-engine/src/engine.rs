//! Compiled rules and their evaluation.

use {
    crate::{
        fee::{self, Caps, Fee},
        file::{RuleDef, RulesFile},
        order_view,
        partner::{self, Signers},
    },
    alloy_primitives::{B256, keccak256},
    cel::{Context, Env, Program, Value},
    chrono::{DateTime, Utc},
    model::order::Order,
    std::{
        collections::{BTreeMap, HashSet},
        fmt,
        sync::Arc,
    },
};

/// What the rules file does not carry.
#[derive(Debug, Clone)]
pub struct Settings {
    pub caps: Caps,
    /// Chain partner attestations are signed for.
    pub chain_id: u64,
}

/// Why a rules file was refused.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("rules file does not parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("partner `{partner}`: no signers")]
    NoSigners { partner: String },
    #[error("rule `{name}` is defined twice")]
    DuplicateRule { name: String },
    #[error("rule `{rule}`: `{field}` does not compile: {message}")]
    Compile {
        rule: String,
        field: Field,
        message: String,
    },
}

/// The two expressions of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    When,
    Fee,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Field::When => "when",
            Field::Fee => "fee",
        })
    }
}

/// One compiled rule.
pub struct Rule {
    name: String,
    when: Expression,
    fee: Expression,
}

impl fmt::Debug for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rule")
            .field("name", &self.name)
            .field("when", &self.when())
            .field("fee", &self.fee())
            .finish()
    }
}

struct Expression {
    program: Program,
    /// Whether the expression reads `size_usd`.
    needs_size_usd: bool,
}

impl Expression {
    fn compile(env: &Env, rule: &str, field: Field, source: &str) -> Result<Self, LoadError> {
        let program = env.compile(source).map_err(|err| LoadError::Compile {
            rule: rule.to_owned(),
            field,
            message: err.to_string(),
        })?;
        Ok(Self {
            needs_size_usd: program.references().has_variable("size_usd"),
            program,
        })
    }

    fn source(&self) -> &str {
        &self.program.source_info().source
    }
}

impl Rule {
    /// Compiles both expressions of a rule definition.
    fn compile(env: &Env, RuleDef { name, when, fee }: RuleDef) -> Result<Self, LoadError> {
        let when = Expression::compile(env, &name, Field::When, &when)?;
        let fee = Expression::compile(env, &name, Field::Fee, &fee)?;
        Ok(Self { name, when, fee })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The condition as written in the rules file.
    pub fn when(&self) -> &str {
        self.when.source()
    }

    /// The fee expression as written in the rules file.
    pub fn fee(&self) -> &str {
        self.fee.source()
    }
}

/// A checked, immutable set of rules.
pub struct RuleSet {
    env: Arc<Env>,
    rules: Vec<Rule>,
    lists: Vec<(String, Value)>,
    signers: Signers,
    settings: Settings,
    source_hash: B256,
}

impl fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuleSet")
            .field("rules", &self.rules)
            .field("source_hash", &self.source_hash)
            .finish_non_exhaustive()
    }
}

impl RuleSet {
    /// Parses and compiles a rules file. Any failure refuses the whole file.
    pub fn load(source: &str, settings: Settings) -> Result<Self, LoadError> {
        let file: RulesFile = toml::from_str(source)?;
        let env = Arc::new(Env::stdlib().with_optional_support(false));
        let lists = list_values(&file.lists);
        let signers = file
            .partners
            .iter()
            .map(|(code, partner)| {
                if partner.signers.is_empty() {
                    return Err(LoadError::NoSigners {
                        partner: code.clone(),
                    });
                }
                Ok((code.clone(), partner.signers.iter().copied().collect()))
            })
            .collect::<Result<Signers, _>>()?;

        let mut rules = Vec::with_capacity(file.rules.len());
        let mut names = HashSet::new();
        for def in file.rules {
            if !names.insert(def.name.clone()) {
                return Err(LoadError::DuplicateRule { name: def.name });
            }
            rules.push(Rule::compile(&env, def)?);
        }

        Ok(Self {
            env,
            rules,
            lists,
            signers,
            settings,
            source_hash: keccak256(source.as_bytes()),
        })
    }

    /// The rules in the order they are tried.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Hash of the rules file, to record which version produced a fee.
    pub fn source_hash(&self) -> B256 {
        self.source_hash
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Binds lists, functions and `now` once for an auction; each order then
    /// gets a cheap child scope.
    pub fn auction(&self, now: DateTime<Utc>) -> Auction<'_> {
        let mut context = Context::with_env(self.env.clone());
        fee::register_fee_functions(&mut context);
        context
            .add_function("max", cel::functions::max)
            .expect("max is not a standard overload");
        context
            .add_function("min", cel::functions::min)
            .expect("min is not a standard overload");
        for (name, list) in &self.lists {
            context.add_variable_from_value(name.clone(), list.clone());
        }
        context.add_variable_from_value("now", now.fixed_offset());
        Auction {
            rules: self,
            context,
        }
    }
}

/// Each `[lists]` entry as the CEL list rules test with `in`, e.g.
/// `order.sellToken in stablecoins`, hex members lowercased like the order
/// view's. `in` scans a list; a member-to-`true` map would make it a key
/// lookup if lists ever grow large.
fn list_values(lists: &BTreeMap<String, Vec<String>>) -> Vec<(String, Value)> {
    lists
        .iter()
        .map(|(name, members)| {
            let members: Vec<String> = members
                .iter()
                .map(|member| order_view::normalize(member))
                .collect();
            (name.clone(), Value::from(members))
        })
        .collect()
}

/// Facts about an order that the order itself does not carry.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Facts {
    /// The order's volume in USD, once market data is loaded.
    pub size_usd: Option<f64>,
}

/// A fact a rule needs that [`Facts`] did not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fact {
    SizeUsd,
}

/// The outcome of running the rules against one order.
#[derive(Debug)]
pub enum Evaluation<'r> {
    /// The first rule whose `when` held, with its fees clamped to the caps.
    Matched { rule: &'r Rule, fees: Vec<Fee> },
    /// No rule matched: the order pays no protocol fee.
    NoMatch,
    /// Evaluation stopped at `rule` because it needs `missing`. Load the fact
    /// and evaluate the order again.
    Suspended { rule: &'r Rule, missing: Fact },
}

/// The rules bound to one auction's shared facts.
pub struct Auction<'r> {
    rules: &'r RuleSet,
    context: Context<'static, 'static>,
}

impl<'r> Auction<'r> {
    /// Runs the rules top to bottom against `order`. A rule that fails to
    /// evaluate is logged and skipped.
    pub fn evaluate(&self, order: &Order, facts: &Facts) -> Evaluation<'r> {
        let app_data = app_data::document_view(order.metadata.full_app_data.as_deref());
        let partner = partner::verified_partner(
            order,
            &app_data,
            &self.rules.signers,
            self.rules.settings.chain_id,
        );
        let mut context = self.context.new_inner_scope();
        context
            .add_variable("order", order_view::order_view(order, app_data))
            .expect("JSON converts to CEL values");
        context.add_variable_from_value("partner", partner);
        if let Some(size_usd) = facts.size_usd {
            context.add_variable_from_value("size_usd", size_usd);
        }

        let suspended = |rule| Evaluation::Suspended {
            rule,
            missing: Fact::SizeUsd,
        };
        for rule in &self.rules.rules {
            if rule.when.needs_size_usd && facts.size_usd.is_none() {
                return suspended(rule);
            }
            let matched = match rule.when.program.execute(&context) {
                Ok(Value::Bool(matched)) => matched,
                Ok(other) => {
                    tracing::warn!(
                        rule = %rule.name,
                        returned = %other.type_of(),
                        "fee rule `when` is not a bool, skipping rule"
                    );
                    false
                }
                Err(err) => {
                    tracing::warn!(rule = %rule.name, ?err, "fee rule `when` failed, skipping rule");
                    false
                }
            };
            if !matched {
                continue;
            }
            if rule.fee.needs_size_usd && facts.size_usd.is_none() {
                return suspended(rule);
            }
            let value = match rule.fee.program.execute(&context) {
                Ok(value) => value,
                Err(err) => {
                    tracing::warn!(rule = %rule.name, ?err, "fee rule `fee` failed, skipping rule");
                    continue;
                }
            };
            let mut fees = match fee::fees_from_value(&value) {
                Ok(fees) => fees,
                Err(err) => {
                    tracing::warn!(rule = %rule.name, %err, "fee rule `fee` is not a fee, skipping rule");
                    continue;
                }
            };
            for fee in &mut fees {
                let before = *fee;
                if fee.clamp(&self.rules.settings.caps) {
                    tracing::warn!(rule = %rule.name, ?before, clamped = ?fee, "fee clamped to the caps");
                }
            }
            return Evaluation::Matched { rule, fees };
        }
        Evaluation::NoMatch
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::partner::PartnerAttestation,
        alloy_primitives::{Address, U256, address, b256, hex},
        alloy_signer::SignerSync,
        alloy_signer_local::PrivateKeySigner,
        chrono::TimeZone,
        configs::fee_factor::FeeFactor,
        model::order::{OrderData, OrderMetadata},
        serde_json::json,
    };

    const USDC: Address = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    const USDT: Address = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7");
    const WETH: Address = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    const MAKER: Address = address!("0x1111111111111111111111111111111111111111");
    /// The address of private key `0x…01`.
    const ACME_SIGNER: Address = address!("0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf");

    const RULES: &str = r#"
[lists]
stablecoins = ["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0xdAC17F958D2ee523a2206206994597C13D831ec7"]
jit_makers = ["0x1111111111111111111111111111111111111111"]

[partners]
acme = { signers = ["0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"] }

[[rule]]
name = "JIT makers"
when = 'order.owner in jit_makers'
fee = 'no_fee()'

[[rule]]
name = "Acme launch month"
when = 'partner == "acme" && now < timestamp("2026-12-01T00:00:00Z")'
fee = 'no_fee()'

[[rule]]
name = "Acme"
when = 'partner == "acme"'
fee = 'volume(size_usd >= 1e6 ? 5 : 10)'

[[rule]]
name = "Aave flashloans"
when = 'order.appData.appCode == "aave" && order.appData.metadata.flashloan != null'
fee = 'volume(20)'

[[rule]]
name = "Stablecoin swaps"
when = 'order.sellToken in stablecoins && order.buyToken in stablecoins'
fee = 'volume(10)'

[[rule]]
name = "Default"
when = 'true'
fee = 'surplus(50, 1)'
"#;

    fn settings() -> Settings {
        Settings {
            caps: Caps::default(),
            chain_id: 1,
        }
    }

    fn load(rules: &str) -> RuleSet {
        RuleSet::load(rules, settings()).unwrap()
    }

    fn order(sell: Address, buy: Address, app_data: Option<serde_json::Value>) -> Order {
        Order {
            metadata: OrderMetadata {
                owner: address!("0x2222222222222222222222222222222222222222"),
                full_app_data: app_data.map(|value| value.to_string()),
                ..Default::default()
            },
            data: OrderData {
                sell_token: sell,
                buy_token: buy,
                sell_amount: U256::from(1_000_000u64),
                buy_amount: U256::from(900_000u64),
                valid_to: 1_800_000_000,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn attested_by_acme(mut order: Order) -> Order {
        let signer = PrivateKeySigner::from_bytes(&b256!(
            "0000000000000000000000000000000000000000000000000000000000000001"
        ))
        .unwrap();
        assert_eq!(signer.address(), ACME_SIGNER);
        let hash = PartnerAttestation::for_order(&order).signing_hash(1);
        let signature = signer.sign_hash_sync(&hash).unwrap();
        order.metadata.full_app_data = Some(
            json!({
                "appCode": "acme",
                "metadata": { "partner": { "signature": hex::encode_prefixed(signature.as_bytes()) } },
            })
            .to_string(),
        );
        order
    }

    fn matched<'r>(evaluation: &Evaluation<'r>) -> (&'r str, Vec<Fee>) {
        match evaluation {
            Evaluation::Matched { rule, fees } => (rule.name(), fees.clone()),
            other => panic!("expected a match, got {other:?}"),
        }
    }

    fn volume(bps: f64) -> Fee {
        Fee::Volume {
            factor: FeeFactor::try_from(bps / 10_000.).unwrap(),
        }
    }

    #[test]
    fn first_matching_rule_wins() {
        let rules = load(RULES);
        let auction = rules.auction(Utc::now());
        let facts = Facts::default();

        let (rule, fees) = matched(&auction.evaluate(&order(USDC, USDT, None), &facts));
        assert_eq!(rule, "Stablecoin swaps");
        assert_eq!(fees, vec![volume(10.)]);

        let (rule, fees) = matched(&auction.evaluate(&order(WETH, USDT, None), &facts));
        assert_eq!(rule, "Default");
        assert_eq!(
            fees,
            vec![Fee::Surplus {
                factor: FeeFactor::try_from(0.5).unwrap(),
                max_volume_factor: FeeFactor::try_from(0.01).unwrap(),
            }]
        );

        let mut jit = order(USDC, USDT, None);
        jit.metadata.owner = MAKER;
        let (rule, fees) = matched(&auction.evaluate(&jit, &facts));
        assert_eq!(rule, "JIT makers");
        assert_eq!(fees, vec![]);

        let aave = order(
            USDC,
            USDT,
            Some(json!({
                "appCode": "aave",
                "metadata": {
                    "flashloan": {
                        "liquidityProvider": MAKER,
                        "protocolAdapter": MAKER,
                        "receiver": MAKER,
                        "token": USDC,
                        "amount": "1",
                    },
                },
            })),
        );
        let (rule, fees) = matched(&auction.evaluate(&aave, &facts));
        assert_eq!(rule, "Aave flashloans");
        assert_eq!(fees, vec![volume(20.)]);
    }

    #[test]
    fn discounts_need_a_verified_partner() {
        let rules = load(RULES);
        let auction = rules.auction(Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap());

        let claimed = order(WETH, USDT, Some(json!({ "appCode": "acme" })));
        let (rule, _) = matched(&auction.evaluate(&claimed, &Facts::default()));
        assert_eq!(rule, "Default");

        let proven = attested_by_acme(order(WETH, USDT, None));
        let evaluation = auction.evaluate(&proven, &Facts::default());
        let Evaluation::Suspended { rule, missing } = &evaluation else {
            panic!("expected a suspension, got {evaluation:?}");
        };
        assert_eq!(rule.name(), "Acme");
        assert_eq!(*missing, Fact::SizeUsd);

        let facts = Facts {
            size_usd: Some(2_000_000.),
        };
        let (rule, fees) = matched(&auction.evaluate(&proven, &facts));
        assert_eq!(rule, "Acme");
        assert_eq!(fees, vec![volume(5.)]);
        let facts = Facts {
            size_usd: Some(1_000.),
        };
        assert_eq!(
            matched(&auction.evaluate(&proven, &facts)).1,
            vec![volume(10.)]
        );
    }

    #[test]
    fn now_is_the_auction_time() {
        let rules = load(RULES);
        let proven = attested_by_acme(order(WETH, USDT, None));
        let launch = rules.auction(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap());
        let (rule, fees) = matched(&launch.evaluate(&proven, &Facts::default()));
        assert_eq!(rule, "Acme launch month");
        assert_eq!(fees, vec![]);
    }

    #[test]
    fn failing_rules_are_skipped() {
        let rules = load(
            r#"
[[rule]]
name = "Broken when"
when = 'order.appData.metadata.flashloan.token == "x"'
fee = 'volume(1)'

[[rule]]
name = "Not a bool"
when = 'order.sellAmount'
fee = 'volume(1)'

[[rule]]
name = "Broken fee"
when = 'true'
fee = 'volume("ten")'

[[rule]]
name = "Default"
when = 'true'
fee = 'volume(2)'
"#,
        );
        let auction = rules.auction(Utc::now());
        let (rule, fees) = matched(&auction.evaluate(&order(WETH, USDT, None), &Facts::default()));
        assert_eq!(rule, "Default");
        assert_eq!(fees, vec![volume(2.)]);
    }

    #[test]
    fn fees_are_clamped_to_the_caps() {
        let rules = load(
            r#"
[[rule]]
name = "Greedy"
when = 'true'
fee = '[volume(500), surplus(95, 10)]'
"#,
        );
        let auction = rules.auction(Utc::now());
        let (_, fees) = matched(&auction.evaluate(&order(WETH, USDT, None), &Facts::default()));
        let caps = Caps::default();
        assert_eq!(
            fees,
            vec![
                Fee::Volume {
                    factor: caps.volume_factor
                },
                Fee::Surplus {
                    factor: caps.share_factor,
                    max_volume_factor: caps.max_volume_factor
                }
            ]
        );
    }

    #[test]
    fn null_is_in_no_list() {
        let rules = load(
            r#"
[lists]
partners = ["acme"]

[[rule]]
name = "Partners"
when = 'partner in partners'
fee = 'volume(1)'

[[rule]]
name = "Default"
when = 'true'
fee = 'volume(2)'
"#,
        );
        let auction = rules.auction(Utc::now());
        let (rule, _) = matched(&auction.evaluate(&order(WETH, USDT, None), &Facts::default()));
        assert_eq!(rule, "Default");
    }

    #[test]
    fn no_rules_means_no_fee() {
        let rules = load("");
        let auction = rules.auction(Utc::now());
        assert!(matches!(
            auction.evaluate(&order(WETH, USDT, None), &Facts::default()),
            Evaluation::NoMatch
        ));
    }

    #[test]
    fn refuses_bad_files() {
        let error = |rules: &str| RuleSet::load(rules, settings()).unwrap_err();

        assert!(matches!(error("[[rule]]\nname = 'x'"), LoadError::Toml(_)));
        assert!(matches!(
            error("[[rule]]\nname = 'x'\nwhen = 'order.sellToken in'\nfee = 'volume(1)'"),
            LoadError::Compile {
                field: Field::When,
                ..
            }
        ));
        assert!(matches!(
            error(
                "[[rule]]\nname = 'x'\nwhen = 'true'\nfee = 'volume(1)'\n[[rule]]\nname = \
                 'x'\nwhen = 'true'\nfee = 'volume(1)'"
            ),
            LoadError::DuplicateRule { .. }
        ));
        assert!(matches!(
            error("[partners]\nacme = { signers = [] }"),
            LoadError::NoSigners { .. }
        ));
    }
}

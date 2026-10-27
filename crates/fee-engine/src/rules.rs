//! Rule definitions and their compilation into a [`RuleSet`].

use {
    crate::{Evaluator, fee, order_view},
    cel::{Context, Env, Program, Value},
    chrono::{DateTime, Utc},
    std::{
        collections::{BTreeMap, HashSet},
        fmt,
        sync::Arc,
    },
};

/// A rule as written: a name and two CEL expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDefinition {
    pub name: String,
    pub when: String,
    pub fee: String,
}

/// Names the evaluator binds itself; a list under one of them would be
/// shadowed or overwritten without any error.
const RESERVED_NAMES: [&str; 3] = ["order", "now", "math"];

/// Why a set of rule definitions was refused.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("rule `{name}` is defined twice")]
    DuplicateRule { name: String },
    #[error("list `{name}` takes a name the engine binds itself")]
    ReservedList { name: String },
    #[error("rule `{rule}`: `{field}` does not compile: {message}")]
    Expression {
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
    pub(crate) when: Program,
    pub(crate) fee: Program,
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

impl Rule {
    /// Compiles both expressions of a definition.
    fn compile(
        env: &Env,
        RuleDefinition { name, when, fee }: RuleDefinition,
    ) -> Result<Self, CompileError> {
        let compile = |field, source: &str| {
            env.compile(source).map_err(|err| CompileError::Expression {
                rule: name.clone(),
                field,
                message: err.to_string(),
            })
        };
        let when = compile(Field::When, &when)?;
        let fee = compile(Field::Fee, &fee)?;
        Ok(Self { name, when, fee })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The condition as written.
    pub fn when(&self) -> &str {
        &self.when.source_info().source
    }

    /// The fee expression as written.
    pub fn fee(&self) -> &str {
        &self.fee.source_info().source
    }
}

/// A checked, immutable set of rules.
pub struct RuleSet {
    pub(crate) env: Arc<Env>,
    rules: Vec<Rule>,
    pub(crate) lists: Vec<(String, Value)>,
}

impl fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuleSet")
            .field("rules", &self.rules)
            .finish_non_exhaustive()
    }
}

impl RuleSet {
    /// Compiles the rules in the order they are tried against the CEL
    /// stdlib plus its math and strings extensions. Each list becomes a CEL
    /// list under its name. Any failure refuses the whole set.
    pub fn new(
        definitions: Vec<RuleDefinition>,
        lists: BTreeMap<String, Vec<String>>,
    ) -> Result<Self, CompileError> {
        let mut env = Env::stdlib().with_optional_support(false);
        cel::extensions::math(&mut env).expect("math names are free in the stdlib");
        cel::extensions::strings(&mut env).expect("strings names are free in the stdlib");
        let env = Arc::new(env);
        let mut rules = Vec::with_capacity(definitions.len());
        if let Some(name) = lists
            .keys()
            .find(|name| RESERVED_NAMES.contains(&name.as_str()))
        {
            return Err(CompileError::ReservedList { name: name.clone() });
        }
        let mut names = HashSet::new();
        for definition in definitions {
            if !names.insert(definition.name.clone()) {
                return Err(CompileError::DuplicateRule {
                    name: definition.name,
                });
            }
            rules.push(Rule::compile(&env, definition)?);
        }
        Ok(Self {
            env,
            rules,
            lists: list_values(&lists),
        })
    }

    /// The rules in the order they are tried.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Binds lists, functions and `now` once; each order evaluated through
    /// the result gets a cheap child scope on top.
    pub fn evaluator(&self, now: DateTime<Utc>) -> Evaluator<'_> {
        let mut context = Context::with_env(self.env.clone());
        fee::register_fee_functions(&mut context);
        for (name, list) in &self.lists {
            context.add_variable_from_value(name.clone(), list.clone());
        }
        context.add_variable_from_value("now", now.fixed_offset());
        Evaluator::new(self, context)
    }
}

/// Each list as the CEL list rules test with `in`, e.g.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str, when: &str, fee: &str) -> RuleDefinition {
        RuleDefinition {
            name: name.to_owned(),
            when: when.to_owned(),
            fee: fee.to_owned(),
        }
    }

    #[test]
    fn keeps_rule_order_and_sources() {
        let rules = RuleSet::new(
            vec![
                rule("First", "order.kind == 'sell'", "volume(1)"),
                rule("Second", "true", "no_fee()"),
            ],
            BTreeMap::new(),
        )
        .unwrap();
        let names: Vec<_> = rules.rules().iter().map(Rule::name).collect();
        assert_eq!(names, ["First", "Second"]);
        assert_eq!(rules.rules()[0].when(), "order.kind == 'sell'");
        assert_eq!(rules.rules()[1].fee(), "no_fee()");
    }

    #[test]
    fn lowercases_hex_list_members() {
        let lists = BTreeMap::from([(
            "tokens".to_owned(),
            vec!["0xAbC".to_owned(), "Acme".to_owned()],
        )]);
        let rules = RuleSet::new(vec![], lists).unwrap();
        let [(name, members)] = &rules.lists[..] else {
            panic!("expected one list");
        };
        assert_eq!(name, "tokens");
        assert_eq!(
            *members,
            Value::from(vec!["0xabc".to_owned(), "Acme".to_owned()])
        );
    }

    #[test]
    fn refuses_bad_definitions() {
        let error = |definitions| RuleSet::new(definitions, BTreeMap::new()).unwrap_err();

        assert!(matches!(
            error(vec![rule("x", "order.sellToken in", "volume(1)")]),
            CompileError::Expression {
                field: Field::When,
                ..
            }
        ));
        assert!(matches!(
            error(vec![rule("x", "true", "volume(")]),
            CompileError::Expression {
                field: Field::Fee,
                ..
            }
        ));
        assert!(matches!(
            error(vec![
                rule("x", "true", "volume(1)"),
                rule("x", "true", "volume(1)")
            ]),
            CompileError::DuplicateRule { .. }
        ));
    }

    #[test]
    fn refuses_reserved_list_names() {
        for name in RESERVED_NAMES {
            let lists = BTreeMap::from([(name.to_owned(), vec![])]);
            assert!(matches!(
                RuleSet::new(vec![], lists).unwrap_err(),
                CompileError::ReservedList { name: found } if found == name
            ));
        }
    }
}

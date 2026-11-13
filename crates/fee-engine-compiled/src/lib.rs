//! The `fee_engine` rules compiled to typed Rust closures instead of run by
//! the CEL interpreter. `cel` only parses; see the README for the subset of
//! CEL it supports and for the exact number types.

mod compile;
mod fee;
mod node;
mod number;
mod order;

use {
    crate::{
        compile::{Scope, compile},
        node::{Set, Val},
        order::Input,
    },
    chrono::{DateTime, Utc},
    fee_engine::{FactorError, Fee, FeeKind, Field, OrderView, RuleSetDefinition},
    std::{collections::HashMap, sync::Arc},
};

/// Why an expression yielded no value for an order; the rule is skipped.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EvalError {
    #[error("the order has no `{0}`")]
    Missing(&'static str),
    #[error("time arithmetic overflowed")]
    Overflow,
    #[error("arithmetic overflowed or divided by zero")]
    Arithmetic,
    #[error("the number does not fit a {0}")]
    Range(&'static str),
    #[error("`{kind}` argument `{argument}` = {value} {error}")]
    Fee {
        kind: FeeKind,
        argument: &'static str,
        value: rust_decimal::Decimal,
        error: FactorError,
    },
}

/// Why a rule set was refused.
#[derive(Debug, thiserror::Error)]
#[error("rule `{rule}`: `{field}` does not compile: {message}")]
pub struct CompileError {
    pub rule: String,
    pub field: Field,
    pub message: String,
}

/// A compiled rule.
pub struct Rule {
    name: String,
    when: Val<bool>,
    fee: Val<Vec<Fee>>,
}

impl Rule {
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Compiled rules, in the order they are tried.
pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    /// Compiles every rule; any failure refuses the whole set.
    /// `partner-fees` is ignored: partner fees are not rules.
    pub fn new(definition: RuleSetDefinition) -> Result<Self, CompileError> {
        let lists: HashMap<String, Arc<Set>> = definition
            .lists
            .iter()
            .map(|(name, members)| {
                let set = Set::new(members.iter().map(String::as_str));
                (name.clone(), Arc::new(set))
            })
            .collect();
        let mut env = cel::Env::stdlib();
        cel::extensions::math(&mut env).expect("math names are free in the stdlib");
        let rules = definition
            .rules
            .into_iter()
            .map(|rule| {
                let error = |field| {
                    let rule = rule.name.clone();
                    move |message| CompileError {
                        rule,
                        field,
                        message,
                    }
                };
                let when = parse(&env, &rule.when)
                    .and_then(|program| compile_program(&program, &lists)?.into_bool())
                    .map_err(error(Field::When))?;
                let fee = parse(&env, &rule.fee)
                    .and_then(|program| compile_program(&program, &lists)?.into_fees())
                    .map_err(error(Field::Fee))?;
                Ok(Rule {
                    name: rule.name,
                    when,
                    fee,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { rules })
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn evaluator(&self, now: DateTime<Utc>) -> Evaluator<'_> {
        Evaluator { rules: self, now }
    }
}

/// Parses with the stdlib and the math extension, as `fee_engine` does.
fn parse(env: &cel::Env, source: &str) -> Result<cel::Program, String> {
    env.compile(source).map_err(|err| err.to_string())
}

/// Compiles a parsed expression; literals are read back from its source.
fn compile_program(
    program: &cel::Program,
    lists: &HashMap<String, Arc<Set>>,
) -> Result<node::Node, String> {
    let scope = Scope {
        lists,
        source: program.source_info(),
    };
    compile(program.expression(), &scope)
}

/// Reads a rules file, as `fee_engine::load_rules_from_str` does.
pub fn load_rules_from_str(source: &str) -> Result<RuleSet, String> {
    let definition: RuleSetDefinition = toml::from_str(source).map_err(|err| err.to_string())?;
    RuleSet::new(definition).map_err(|err| err.to_string())
}

/// What the rules said about one order.
#[derive(Debug)]
pub struct Evaluation<'r> {
    pub matched: Vec<Match<'r>>,
    pub skipped: Vec<Skipped<'r>>,
}

#[derive(Debug)]
pub struct Match<'r> {
    pub rule: &'r Rule,
    pub fees: Vec<Fee>,
}

#[derive(Debug)]
pub struct Skipped<'r> {
    pub rule: &'r Rule,
    pub field: Field,
    pub error: EvalError,
}

impl std::fmt::Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// A [`RuleSet`] at a fixed `now`.
pub struct Evaluator<'r> {
    rules: &'r RuleSet,
    now: DateTime<Utc>,
}

impl<'r> Evaluator<'r> {
    /// Runs every rule against `order`, as `fee_engine::Evaluator::evaluate`.
    pub fn evaluate(&self, order: OrderView) -> Evaluation<'r> {
        let input = Input {
            order,
            now: self.now,
        };
        let mut evaluation = Evaluation {
            matched: Vec::new(),
            skipped: Vec::new(),
        };
        for rule in &self.rules.rules {
            match rule.when.eval(&input) {
                Ok(false) => {}
                Ok(true) => match rule.fee.eval(&input) {
                    Ok(fees) => evaluation.matched.push(Match { rule, fees }),
                    Err(error) => evaluation.skipped.push(Skipped {
                        rule,
                        field: Field::Fee,
                        error,
                    }),
                },
                Err(error) => evaluation.skipped.push(Skipped {
                    rule,
                    field: Field::When,
                    error,
                }),
            }
        }
        evaluation
    }
}

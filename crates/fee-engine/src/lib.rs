//! Rule-based protocol fees.
//!
//! A [`RuleSet`] is an ordered list of rules plus named lists. A rule is a
//! name, a CEL `when` condition and a CEL `fee` expression. For every order
//! the engine reports each rule whose `when` is true, in rule order, with its
//! fee; the caller picks which to act on.
//!
//! A rule sees `order` (the fee-relevant fields of the API order, amounts as
//! doubles and hex in lowercase), `now` and every list. `README.md` documents
//! every name a rule can use, with an example each, and the TOML file
//! [`load_rules`] reads.

mod evaluator;
mod fee;
mod loader;
mod order_view;
mod rules;

pub use {
    evaluator::{Evaluation, Evaluator, Match, RuleError, Skipped},
    fee::Fee,
    loader::{LoadError, load_rules},
    order_view::Order,
    rules::{CompileError, Field, PreflightError, Rule, RuleDefinition, RuleSet},
};

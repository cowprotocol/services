//! Rule-based protocol fees.
//!
//! A [`RuleSet`] is an ordered list of rules plus named lists. A rule is a
//! name, a CEL `when` condition and a CEL `fee` expression. For every order
//! the rules run top to bottom and the first `when` that is true picks the
//! fee.
//!
//! A rule sees `order` (the API order with decimal strings as doubles and
//! hex in lowercase), `now` and every list. `README.md` documents every name a
//! rule can use, with an example each.

mod evaluator;
mod fee;
mod order_view;
mod rules;

pub use {
    evaluator::{Evaluator, Match},
    fee::Fee,
    rules::{CompileError, Field, Rule, RuleDefinition, RuleSet},
};

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
//! [`load_rules_from_str`] reads.

mod evaluator;
mod fee;
mod fee_factor;
mod loader;
mod order_view;
mod partner_fee;
mod report;
mod rules;

pub use {
    evaluator::{Evaluation, Evaluator, Match, RuleError, Skipped},
    fee::{FactorError, Fee, FeeError, FeeKind},
    fee_factor::FeeFactor,
    loader::{LoadError, RulesToml, load_rules_from_str, load_rules_toml},
    order_view::{AppData, Flashloan, OrderView},
    partner_fee::{PartnerFee, PartnerFeeBudget},
    report::{Mismatch, Outcome, report_comparison, reset_comparison_counters},
    rules::{
        CompileError,
        DEFAULT_MAX_VOLUME_BPS,
        Field,
        PartnerFees,
        Rule,
        RuleDefinition,
        RuleSet,
        RuleSetDefinition,
        Sample,
        SampleOrderError,
    },
};

//! Every `toml` block in the README is a rules file that loads, and every
//! `when:` / `fee:` example line compiles.

use {
    fee_engine::{RuleDefinition, RuleSet, RuleSetDefinition},
    std::collections::BTreeMap,
};

const README: &str = include_str!("../README.md");

#[test]
fn readme_examples_load() {
    let blocks: Vec<&str> = README
        .split("```toml\n")
        .skip(1)
        .map(|chunk| chunk.split("```").next().unwrap())
        .collect();
    assert!(!blocks.is_empty());
    for (index, block) in blocks.iter().enumerate() {
        fee_engine::load_rules_from_str(block)
            .unwrap_or_else(|err| panic!("README block {index}: {err}"));
    }
}

#[test]
fn readme_expression_examples_compile() {
    let lists: BTreeMap<_, _> = ["stablecoins", "partner_apps", "jit_makers"]
        .into_iter()
        .map(|name| (name.to_owned(), vec![]))
        .collect();
    let mut found = 0;
    for line in README.lines() {
        let (when, fee) = if let Some(when) = line.strip_prefix("when:") {
            (when.trim(), "no_fee()")
        } else if let Some(fee) = line.strip_prefix("fee:") {
            ("true", fee.trim())
        } else {
            continue;
        };
        found += 1;
        RuleSet::new(RuleSetDefinition {
            lists: lists.clone(),
            rules: vec![RuleDefinition {
                name: "example".to_owned(),
                when: when.to_owned(),
                fee: fee.to_owned(),
            }],
            ..Default::default()
        })
        .unwrap_or_else(|err| panic!("README line `{line}`: {err}"));
    }
    assert!(found > 10, "{found}");
}

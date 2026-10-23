//! Every `toml` block in the README is a rules file that loads.

use fee_engine::{Caps, RuleSet, Settings};

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
        let settings = Settings {
            caps: Caps::default(),
        };
        RuleSet::load(block, settings).unwrap_or_else(|err| panic!("README block {index}: {err}"));
    }
}

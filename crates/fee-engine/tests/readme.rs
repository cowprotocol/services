//! Every `toml` block in the README is a rules file that loads.

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

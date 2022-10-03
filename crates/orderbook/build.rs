use anyhow::Result;
use vergen::{vergen, Config, SemverKind};

fn main() -> Result<()> {
    let mut config = Config::default();
    // Github releases create lightweight tags (not annotated tags) meaning we need to adjust the SEMVER kind in order to see our tag names
    // Related issue: https://github.com/orgs/community/discussions/4924
    *config.git_mut().semver_kind_mut() = SemverKind::Lightweight;
    vergen(config)
}

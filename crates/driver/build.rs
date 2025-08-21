use {
    anyhow::{Context, Result},
    vergen::EmitBuilder,
};

fn main() -> Result<()> {
    // Set environment variable VERGEN_GIT_DESCRIBE for use to log version at startup
    EmitBuilder::builder()
        .git_describe(true, true, None)
        .emit()
        .context("emit")
}

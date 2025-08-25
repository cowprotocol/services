use {
    anyhow::{Context, Result},
    vergen::EmitBuilder,
};

fn main() -> Result<()> {
    // Set environment variable VERGEN_GIT_SHA for use to log version at startup
    EmitBuilder::builder().git_sha(true).emit().context("emit")
}

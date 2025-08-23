use {
    anyhow::{Context, Result},
    vergen::EmitBuilder,
};

fn main() -> Result<()> {
    // Set environment variable VERGEN_GIT_SHA for use to log version at startup
    EmitBuilder::builder()
        .git_sha(true)
        .emit()
        .context("emit")?;

    // Handle allocator selection based on _RJEM_MALLOC_CONF environment variable
    // If _RJEM_MALLOC_CONF is set, use jemalloc; otherwise use mimalloc
    if std::env::var("_RJEM_MALLOC_CONF").is_ok() {
        println!("cargo:rustc-cfg=feature=\"jemalloc-allocator\"");
    } else {
        println!("cargo:rustc-cfg=feature=\"mimalloc-allocator\"");
    }

    println!("cargo:rerun-if-env-changed=_RJEM_MALLOC_CONF");

    Ok(())
}

use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, clap::Parser)]
pub struct Args {
    /// The address to bind the driver to.
    #[clap(long, env, default_value = "0.0.0.0:11098")]
    pub addr: SocketAddr,

    /// Path to the driver configuration file. This file should be in TOML
    /// format.
    #[clap(long, env)]
    pub config: PathBuf,
}

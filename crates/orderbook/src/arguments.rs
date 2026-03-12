use {
    price_estimation,
    std::{net::SocketAddr, path::PathBuf},
};

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(flatten)]
    pub shared: shared::arguments::Arguments,

    #[clap(flatten)]
    pub price_estimation: price_estimation::Arguments,

    /// Path to the TOML configuration file.
    #[clap(long, env)]
    pub config: PathBuf,

    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Arguments {
            shared,
            price_estimation,
            config,
            bind_address,
        } = self;

        write!(f, "{shared}")?;
        write!(f, "{price_estimation}")?;
        writeln!(f, "config: {}", config.display())?;
        writeln!(f, "bind_address: {bind_address}")?;

        Ok(())
    }
}

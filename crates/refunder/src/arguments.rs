use clap::Parser;
use std::time::Duration;
use url::Url;

#[derive(Parser)]
pub struct Arguments {
    /// Minimum time an order must have been valid for, in order
    /// to be eligble for refunding
    #[clap(
        long,
        env,
        default_value = "120",
        value_parser = shared::arguments::duration_from_seconds,
    )]
    pub min_validity_duration: Duration,

    /// Minimum slippage an order must have, in order
    /// to be eligble for refunding
    /// Front-end will place orders with a default slippage of 2% 
    /// hence, we are requiring as a default 1.9%
    #[clap(
        long,
        env,
        default_value = "0.019",
    )]
    pub min_slippage: f64,

    /// Url of the Postgres database. By default connects to locally running postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "min_validity_duration: {:?}", self.min_validity_duration)?;
        writeln!(f, "min_slippage: {}", self.min_slippage)?;
        writeln!(f, "db_url: SECRET")?;
        Ok(())
    }
}

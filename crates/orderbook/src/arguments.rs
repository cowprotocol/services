use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Arguments {
    /// Path to the TOML configuration file.
    #[clap(long, env)]
    pub config: PathBuf,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Arguments { config } = self;

        writeln!(f, "config: {}", config.display())?;

        Ok(())
    }
}

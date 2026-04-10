use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct CliArguments {
    #[clap(long, env)]
    pub config: PathBuf,
}

impl std::fmt::Display for CliArguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { config } = self;
        write!(f, "{}", config.display())?;
        Ok(())
    }
}

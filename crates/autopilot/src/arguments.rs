use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct CliArguments {
    #[clap(long, env)]
    pub config: PathBuf,

    #[clap(flatten)]
    pub shared: shared::arguments::Arguments,

    #[clap(flatten)]
    pub price_estimation: price_estimation::Arguments,
}

impl std::fmt::Display for CliArguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            config,
            shared,
            price_estimation,
        } = self;
        write!(f, "{}", config.display())?;
        write!(f, "{shared}")?;
        write!(f, "{price_estimation}")?;
        Ok(())
    }
}

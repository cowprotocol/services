use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env)]
    pub config: PathBuf,
}

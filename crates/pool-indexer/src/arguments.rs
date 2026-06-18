use {shared::logging_args_with_default_filter, std::path::PathBuf};

logging_args_with_default_filter!(LoggingArguments, "warn,pool_indexer=debug,shared=debug");

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env)]
    pub config: PathBuf,

    #[clap(flatten)]
    pub logging: LoggingArguments,
}

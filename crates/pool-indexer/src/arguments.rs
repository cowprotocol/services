use {shared::logging_args_with_default_filter, std::path::PathBuf};

logging_args_with_default_filter!(LoggingArguments, "warn,pool_indexer=debug,shared=debug");

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env)]
    pub config: PathBuf,

    /// Run only the bootstrap phase (initial seed + catch-up to the finalized
    /// head), then exit; bind no HTTP ports. Idempotent: a fast no-op when the
    /// DB is already seeded. Lets the slow seed run as a separate step ahead of
    /// serving.
    #[clap(long, env, action = clap::ArgAction::Set, default_value_t = false)]
    pub bootstrap_only: bool,

    #[clap(flatten)]
    pub logging: LoggingArguments,
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn bootstrap_only_flag_parses() {
        let serve = Arguments::parse_from(["pool-indexer", "--config", "/tmp/c.toml"]);
        assert!(!serve.bootstrap_only);

        let bootstrap = Arguments::parse_from([
            "pool-indexer",
            "--config",
            "/tmp/c.toml",
            "--bootstrap-only",
            "true",
        ]);
        assert!(bootstrap.bootstrap_only);

        // ArgAction::Set takes a value, so it can be explicitly turned off too.
        let explicit_serve = Arguments::parse_from([
            "pool-indexer",
            "--config",
            "/tmp/c.toml",
            "--bootstrap-only",
            "false",
        ]);
        assert!(!explicit_serve.bootstrap_only);
    }
}

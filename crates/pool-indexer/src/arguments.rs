use {shared::logging_args_with_default_filter, std::path::PathBuf};

logging_args_with_default_filter!(LoggingArguments, "warn,pool_indexer=debug,shared=debug");

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env)]
    pub config: PathBuf,

    /// Run only the bootstrap phase (initial seed + catch-up to the finalized
    /// head), then exit; bind no HTTP ports. Idempotent: a fast no-op when the
    /// DB is already seeded. Lets K8s run bootstrap as an initContainer and
    /// apply tight startup probes to the serve container.
    #[clap(long, env)]
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
        ]);
        assert!(bootstrap.bootstrap_only);
    }
}

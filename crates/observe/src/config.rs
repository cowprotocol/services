use tracing::level_filters::LevelFilter;

#[derive(Debug, Clone)]
pub struct ObserveConfig {
    /// https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html
    pub env_filter: String,
    /// Minimum level threshold for stderr output
    pub stderr_threshold: LevelFilter,
    /// Output log events as JSON
    pub use_json_format: bool,
    /// Tracing config
    pub tracing: Option<TracingConfig>,
}

impl ObserveConfig {
    pub fn new(
        env_filter: &str,
        stderr_threshold: LevelFilter,
        use_json_format: bool,
        tracing_config: Option<TracingConfig>,
    ) -> Self {
        Self {
            env_filter: env_filter.into(),
            stderr_threshold,
            use_json_format,
            tracing: tracing_config,
        }
    }

    /// Create an ObserveConfig with JSON format enabled
    pub fn with_json_format(mut self) -> Self {
        self.use_json_format = true;
        self
    }

    pub fn with_env_filter(mut self, env_filter: &str) -> Self {
        self.env_filter = env_filter.to_string();
        self
    }

    pub fn with_stderr_threshold(mut self, stderr_threshold: LevelFilter) -> Self {
        self.stderr_threshold = stderr_threshold;
        self
    }

    pub fn with_tracing(mut self, tracing_config: TracingConfig) -> Self {
        self.tracing = Some(tracing_config);
        self
    }
}

impl Default for ObserveConfig {
    fn default() -> Self {
        Self {
            env_filter: "info".to_string(),
            stderr_threshold: LevelFilter::ERROR,
            use_json_format: false,
            tracing: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TracingConfig {
    /// Endpoint to send tracing info to
    pub collector_endpoint: String,
    /// Service name which will appear in spans
    pub service_name: String,
}

impl TracingConfig {
    pub fn new(collector_endpoint: String, service_name: String) -> Self {
        Self {
            collector_endpoint,
            service_name,
        }
    }
}

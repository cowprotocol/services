use {core::time::Duration, tracing::Level};

#[derive(Debug, Clone)]
pub struct Config {
    /// Filters spans and events based on a set of filter directives
    /// https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html
    pub(crate) env_filter: String,
    /// Minimum level threshold for stderr output
    pub(crate) stderr_threshold: Option<Level>,
    /// Output log events as JSON
    pub(crate) use_json_format: bool,
    /// Tracing config
    pub(crate) tracing: Option<TracingConfig>,
}

impl Config {
    pub fn new(
        env_filter: &str,
        stderr_threshold: Option<Level>,
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

    pub fn with_stderr_threshold(mut self, stderr_threshold: Level) -> Self {
        self.stderr_threshold = Some(stderr_threshold);
        self
    }

    pub fn with_tracing(mut self, tracing_config: TracingConfig) -> Self {
        self.tracing = Some(tracing_config);
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            env_filter: "info".to_string(),
            stderr_threshold: None,
            use_json_format: false,
            tracing: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Endpoint to send tracing info to
    pub(crate) collector_endpoint: String,
    /// Service name which will appear in spans
    pub(crate) service_name: String,
    /// Timeout for exporting spans to collector
    pub(crate) export_timeout: Duration,
    /// Level of traces that should be collected
    pub(crate) level: Level,
}

impl TracingConfig {
    pub fn new(
        collector_endpoint: String,
        service_name: String,
        export_timeout: Duration,
        level: Level,
    ) -> Self {
        Self {
            collector_endpoint,
            service_name,
            export_timeout,
            level,
        }
    }
}

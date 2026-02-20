use {
    crate::{
        config::Config,
        distributed_tracing::{
            request_id::RequestIdLayer,
            trace_id_format::{TraceIdFmt, TraceIdJsonFormat},
        },
        tracing_reload_handler::spawn_reload_handler,
    },
    axum::http::{self, HeaderMap},
    opentelemetry::{
        Context,
        KeyValue,
        global,
        propagation::{Extractor, Injector},
        trace::TracerProvider,
    },
    opentelemetry_otlp::WithExportConfig,
    opentelemetry_sdk::{
        Resource,
        propagation::TraceContextPropagator,
        trace::{RandomIdGenerator, Sampler},
    },
    std::{panic::PanicHookInfo, sync::Once},
    time::macros::format_description,
    tracing::{Span, level_filters::LevelFilter},
    tracing_opentelemetry::OpenTelemetrySpanExt,
    tracing_subscriber::{
        EnvFilter,
        Layer,
        fmt::{time::UtcTime, writer::MakeWriterExt as _},
        prelude::*,
        util::SubscriberInitExt,
    },
};

/// Initializes tracing setup that is shared between the binaries.
/// `env_filter` has similar syntax to env_logger. It is documented at
/// https://docs.rs/tracing-subscriber/0.2.15/tracing_subscriber/filter/struct.EnvFilter.html
pub fn initialize(config: &Config) {
    set_tracing_subscriber(config);
    std::panic::set_hook(Box::new(tracing_panic_hook));
}

/// Like [`initialize`], but can be called multiple times in a row. Later calls
/// are ignored.
///
/// Useful for tests.
pub fn initialize_reentrant(config: &Config) {
    // The tracing subscriber below is global object so initializing it again in the
    // same process by a different thread would fail.
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        set_tracing_subscriber(config);
        std::panic::set_hook(Box::new(tracing_panic_hook));
    });
}

fn set_tracing_subscriber(config: &Config) {
    let initial_filter = config.env_filter.to_string();

    // The `tracing` APIs are heavily generic to enable zero overhead.

    macro_rules! fmt_layer {
        ($env_filter:expr_2021, $stderr_threshold:expr_2021, $use_json_format:expr_2021) => {{
            let stderr_threshold = $stderr_threshold.clone();
            let writer = std::io::stderr
                .with_filter(move |meta| {
                    // if the log is at most as verbose as our stderr threshold we log it to
                    // stderr. For example if the threshold is WARN all WARN and ERROR logs
                    // will get written to stderr.
                    stderr_threshold.is_some_and(|min_verbosity| meta.level() <= &min_verbosity)
                })
                .or_else(std::io::stdout);
            let timer = UtcTime::new(format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
            ));

            if config.use_json_format {
                // structured logging
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .fmt_fields(tracing_subscriber::fmt::format::JsonFields::default())
                    .event_format(TraceIdJsonFormat)
                    .with_writer(writer)
                    .with_filter($env_filter)
                    .boxed()
            } else {
                let is_terminal = std::io::IsTerminal::is_terminal(&std::io::stdout());
                tracing_subscriber::fmt::layer()
                    .with_timer(timer)
                    .with_ansi(is_terminal)
                    .map_event_format(|formatter| TraceIdFmt {
                        inner: formatter.with_ansi(is_terminal),
                    })
                    .with_writer(writer)
                    .with_filter($env_filter)
                    .boxed()
            }
        }};
    }

    let (env_filter, reload_handle) =
        tracing_subscriber::reload::Layer::new(EnvFilter::new(&initial_filter));

    let tracing_layer = if let Some(tracing_config) = &config.tracing {
        global::set_text_map_propagator(TraceContextPropagator::new());

        let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(tracing_config.collector_endpoint.as_str())
            .with_timeout(tracing_config.export_timeout)
            .build()
            .expect("otlp exporter");
        let tracer = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(otlp_exporter)
            .with_sampler(Sampler::AlwaysOn) // TODO figure out sampling + make configurable
            .with_id_generator(RandomIdGenerator::default())
            .with_resource(
                Resource::builder()
                    .with_attribute(KeyValue::new(
                        "service.name",
                        tracing_config.service_name.to_owned(),
                    ))
                    .build(),
            )
            .build()
            .tracer("cow_tracing");
        tracing::info!("tracing layer set up");
        Some(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(LevelFilter::from_level(tracing_config.level)),
        )
    } else {
        tracing::info!("no tracing layer set up");
        None
    };

    let subscriber = tracing_subscriber::registry()
        .with(LevelFilter::TRACE)
        .with(RequestIdLayer)
        .with(fmt_layer!(
            env_filter,
            config.stderr_threshold,
            config.use_json_format
        ))
        .with(tracing_layer);

    #[cfg(all(tokio_unstable, feature = "tokio-console"))]
    {
        let enable_tokio_console: bool = std::env::var("TOKIO_CONSOLE")
            .unwrap_or("false".to_string())
            .parse()
            .unwrap();

        if enable_tokio_console {
            subscriber.with(console_subscriber::spawn()).init();
            tracing::info!("started program with support for tokio-console");
        } else {
            subscriber.init();
            tracing::info!(
                "started program without support for tokio-console (TOKIO_CONSOLE=false)"
            );
        }
    }

    #[cfg(not(all(tokio_unstable, feature = "tokio-console")))]
    {
        subscriber.init();
        tracing::info!(
            "started program without support for tokio-console (not compiled with tokio_unstable \
             cfg and tokio-console feature)"
        );
    }

    if cfg!(unix) {
        spawn_reload_handler(initial_filter, reload_handle);
    }
}

/// Panic hook that prints roughly the same message as the default panic hook
/// but uses tracing:error instead of stderr.
///
/// Useful when we want panic messages to have the proper log format for Kibana.
fn tracing_panic_hook(panic: &PanicHookInfo) {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("<unnamed>");
    let backtrace = std::backtrace::Backtrace::force_capture();
    tracing::error!("thread '{name}' {panic}\nstack backtrace:\n{backtrace}");
}

pub struct HeaderExtractor<'a>(pub &'a HeaderMap);

// Copied from https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-http/src/lib.rs
// because that crate is using `http` crate v1 while warp is on v0.2
impl Extractor for HeaderExtractor<'_> {
    /// Get a value for a key from the HeaderMap.  If the value is not valid
    /// ASCII, returns None.
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    /// Collect all the keys from the HeaderMap.
    fn keys(&self) -> Vec<&str> {
        self.0
            .keys()
            .map(|value| value.as_str())
            .collect::<Vec<_>>()
    }
}

pub struct HeaderInjector<'a>(pub &'a mut http::HeaderMap);

impl Injector for HeaderInjector<'_> {
    /// Set a key and value in the HeaderMap. Does nothing if the key or value
    /// are not valid inputs.
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(val)) = (
            http::header::HeaderName::from_bytes(key.as_bytes()),
            http::header::HeaderValue::from_str(&value),
        ) {
            self.0.insert(name, val);
        }
    }
}

pub fn tracing_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    Context::current();
    let span = Span::current();
    let cx = span.context();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut HeaderInjector(&mut headers))
    });

    headers
}

/// Helper struct to lazily evaluate an expression if a log is actually active.
/// Sometimes you need to compute a value for logs. This expression gets
/// evaluated eagerly in the `tracing` log macros. In order to only evaluate the
/// expression when the log is actually enabled wrap the expression in a closure
/// and wrap that in a [`Lazy`].
pub struct Lazy<T>(pub T);

impl<T, D> std::fmt::Debug for Lazy<T>
where
    T: Fn() -> D,
    D: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", (self.0)())
    }
}

impl<T, D> std::fmt::Display for Lazy<T>
where
    T: Fn() -> D,
    D: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", (self.0)())
    }
}

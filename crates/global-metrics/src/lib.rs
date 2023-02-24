use {once_cell::sync::OnceCell, std::collections::HashMap};

/// Global metrics registry used by all components.
static REGISTRY: OnceCell<prometheus_metric_storage::StorageRegistry> = OnceCell::new();

/// Configure global metrics registry.
///
/// This function allows specifying common prefix that will be added
/// to all metric names, as well as common labels.
///
/// This function can be called at most once, and it should be done before
/// any call to [`get_registry`], ideally in the very beginning
/// of the `main` function.
///
/// # Panics
///
/// This function panics if it's called twice, or if it's called after
/// any call to [`get_registry`]. This function also panics if registry
/// configuration is invalid.
pub fn setup_metrics_registry(prefix: Option<String>, labels: Option<HashMap<String, String>>) {
    let registry = prometheus::Registry::new_custom(prefix, labels).unwrap();
    let storage_registry = prometheus_metric_storage::StorageRegistry::new(registry);
    REGISTRY.set(storage_registry).unwrap();
}

/// Get the global instance of the metrics registry.
pub fn get_metrics_registry() -> &'static prometheus::Registry {
    get_metric_storage_registry().registry()
}

/// Get the global instance of the metric storage registry.
///
/// # Implementation notice
///
/// If global metrics registry was not configured with [`setup_registry`],
/// it will be initialized using a default value. We could've panic instead,
/// but panicking creates troubles for unit tests. There is no way to set up
/// a hook that will call [`setup_registry`] before each test, so we'll
/// have to initialize it manually before every test, which is tedious
/// to say the least.
pub fn get_metric_storage_registry() -> &'static prometheus_metric_storage::StorageRegistry {
    REGISTRY.get_or_init(prometheus_metric_storage::StorageRegistry::default)
}

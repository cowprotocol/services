//! Global `prometheus-client` registry for metrics migrated away from the
//! `prometheus` crate. [`crate::metrics`] serves it after its own output.

use {
    prometheus_client::registry::Registry,
    std::sync::{Mutex, OnceLock, PoisonError},
};

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

/// Configures the global registry with a prefix for all metric names.
///
/// # Panics
///
/// Panics if it's called twice, or after any call to [`register`].
pub fn setup_registry(prefix: Option<String>) {
    let registry = prefix.map(Registry::with_prefix).unwrap_or_default();
    REGISTRY
        .set(Mutex::new(registry))
        .unwrap_or_else(|_| panic!("client metrics registry already initialized"));
}

/// Registers metrics in the global registry, e.g. from the `OnceLock` that
/// holds a metrics struct. The lock is not re-entrant, so the closure must not
/// initialize other metrics structs.
pub fn register(register: impl FnOnce(&mut Registry)) {
    let registry = REGISTRY.get_or_init(Default::default);
    register(&mut registry.lock().unwrap_or_else(PoisonError::into_inner));
}

/// Encodes the global registry without the OpenMetrics EOF marker, so the
/// output can follow the `prometheus` text output. Empty if never used.
pub fn encode() -> String {
    REGISTRY
        .get()
        .map(|registry| encode_registry(&registry.lock().unwrap_or_else(PoisonError::into_inner)))
        .unwrap_or_default()
}

fn encode_registry(registry: &Registry) -> String {
    let mut text = String::new();
    prometheus_client::encoding::text::encode_registry(&mut text, registry).unwrap();
    text
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        prometheus_client::metrics::{counter::Counter, family::Family},
    };

    #[test]
    fn encodes_without_eof() {
        let mut registry = Registry::with_prefix("refunder");
        let family = Family::<Vec<(&'static str, &'static str)>, Counter>::default();
        family.get_or_create(&vec![("result", "error")]).inc();
        registry
            .sub_registry_with_prefix("main")
            .register("loops", "Client counter", family);

        let text = encode_registry(&registry);
        assert!(
            text.contains("# TYPE refunder_main_loops counter\n"),
            "{text}"
        );
        assert!(
            text.contains("refunder_main_loops_total{result=\"error\"} 1\n"),
            "{text}"
        );
        assert!(!text.contains("# EOF"), "{text}");
    }
}

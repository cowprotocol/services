/// Note: cloning shares the underlying cache instance.
#[derive(Clone)]
pub(crate) struct Cache<K, V> {
    data: moka::sync::Cache<K, V>,
}

impl<K, V> Cache<K, V>
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub(crate) fn new(max_capacity: u64) -> Self {
        let data = moka::sync::Cache::builder()
            .max_capacity(max_capacity)
            .build();
        Self { data }
    }

    pub(crate) fn get(&self, key: &K) -> Option<V> {
        self.data.get(key)
    }

    pub(crate) fn insert(&self, key: K, value: V) {
        self.data.insert(key, value)
    }
}

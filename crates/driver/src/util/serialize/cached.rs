use {
    serde::{Serialize, Serializer},
    serde_json::value::{to_raw_value, RawValue},
    std::{any::type_name, fmt, marker::PhantomData, sync::Arc},
};

/// Wrapper around [`serde_json::value::RawValue`] that allows caching a
/// serialized representation of a value while remaining ergonomic to use with
/// Serde.
#[derive(Clone)]
pub struct Cached<T> {
    raw: Arc<RawValue>,
    _marker: PhantomData<T>,
}

impl<T> Cached<T>
where
    T: Serialize,
{
    /// Serializes `value` to JSON once and stores the resulting raw
    /// representation for cheap cloning and serialization in the future.
    pub fn new(value: T) -> Self {
        Self::try_new(value).unwrap_or_else(|err| {
            panic!(
                "failed to serialize `{}` into cached JSON: {err}",
                type_name::<T>()
            )
        })
    }

    /// Fallible variant of [`Self::new`].
    pub fn try_new(value: T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            raw: Arc::from(to_raw_value(&value)?),
            _marker: PhantomData,
        })
    }

    /// Returns the underlying raw JSON string.
    pub fn as_str(&self) -> &str {
        self.raw.get()
    }
}

impl<T> Serialize for Cached<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.raw.serialize(serializer)
    }
}

impl<T> fmt::Debug for Cached<T>
where
    T: Serialize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Cached").field(&self.as_str()).finish()
    }
}

impl<T> Default for Cached<T>
where
    T: Default + Serialize,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// A thin wrapper around a collection of bytes. Provides hex debug formatting.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bytes<T>(pub T);

impl<T> std::fmt::Debug for Bytes<T>
where
    T: AsRef<[u8]>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

impl<T> From<T> for Bytes<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

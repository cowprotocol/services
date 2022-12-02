pub struct NetworkName(pub String);

pub struct ChainId(pub u64);

impl From<String> for NetworkName {
    fn from(inner: String) -> Self {
        Self(inner)
    }
}

impl From<u64> for ChainId {
    fn from(inner: u64) -> Self {
        Self(inner)
    }
}

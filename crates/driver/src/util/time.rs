/// A Unix timestamp denominated in seconds since epoch.
///
/// https://en.wikipedia.org/wiki/Unix_time
#[derive(Debug, Clone, Copy)]
pub struct Timestamp(pub u32);

impl From<u32> for Timestamp {
    fn from(inner: u32) -> Self {
        Self(inner)
    }
}

impl From<Timestamp> for u32 {
    fn from(timestamp: Timestamp) -> Self {
        timestamp.0
    }
}

impl Timestamp {
    pub const MAX: Self = Self(u32::MAX);
}

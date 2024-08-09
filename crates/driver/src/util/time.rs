use derive_more::{From, Into};

/// A Unix timestamp denominated in seconds since epoch.
///
/// https://en.wikipedia.org/wiki/Unix_time
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct Timestamp(pub u32);

impl Timestamp {
    pub const MAX: Self = Self(u32::MAX);
}

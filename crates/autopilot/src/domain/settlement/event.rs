use crate::domain::eth;

#[derive(Debug, Clone, Copy)]
pub struct LogIndex(pub u64);

impl From<u64> for LogIndex {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Event emitted by the settlement contract.
#[derive(Debug)]
pub struct Event {
    pub block: eth::BlockNo,
    pub log: LogIndex,
}

pub mod instrumented;
pub mod list_based;
pub mod trace_call;

use {alloy::primitives::Address, anyhow::Result};

/// How well behaved a token is.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TokenQuality {
    Good,
    Bad { reason: String },
}

impl TokenQuality {
    pub fn is_good(&self) -> bool {
        matches!(self, Self::Good { .. })
    }

    pub fn bad(reason: impl ToString) -> Self {
        Self::Bad {
            reason: reason.to_string(),
        }
    }
}

/// Detect how well behaved a token is.
#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[async_trait::async_trait]
pub trait BadTokenDetecting: Send + Sync {
    async fn detect(&self, token: Address) -> Result<TokenQuality>;
}

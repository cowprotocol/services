pub mod cache;
pub mod instrumented;
pub mod list_based;
pub mod token_owner_finder;
pub mod trace_call;

use {anyhow::Result, primitive_types::H160};

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
#[mockall::automock]
#[async_trait::async_trait]
pub trait BadTokenDetecting: Send + Sync {
    async fn detect(&self, token: H160) -> Result<TokenQuality>;
}

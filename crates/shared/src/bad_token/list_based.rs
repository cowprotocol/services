use super::{BadTokenDetecting, TokenQuality};
use anyhow::Result;
use primitive_types::H160;

/// If a token is neither in the allow nor the deny list treat it this way.
pub enum UnknownTokenStrategy {
    Allow,
    Deny,
    Forward(Box<dyn BadTokenDetecting>),
}

/// Classify tokens with explicit allow and deny lists.
pub struct ListBasedDetector {
    allow_list: Vec<H160>,
    deny_list: Vec<H160>,
    strategy: UnknownTokenStrategy,
}

impl ListBasedDetector {
    /// Panics if same token is both allowed and denied.
    pub fn new(
        allow_list: Vec<H160>,
        deny_list: Vec<H160>,
        strategy: UnknownTokenStrategy,
    ) -> Self {
        assert!(
            allow_list.iter().all(|token| !deny_list.contains(token)),
            "token is allowed and denied"
        );
        Self {
            allow_list,
            deny_list,
            strategy,
        }
    }

    pub fn deny_list(list: Vec<H160>) -> Self {
        Self {
            allow_list: Vec::new(),
            deny_list: list,
            strategy: UnknownTokenStrategy::Allow,
        }
    }
}

#[async_trait::async_trait]
impl BadTokenDetecting for ListBasedDetector {
    async fn detect(&self, token: ethcontract::H160) -> Result<TokenQuality> {
        if self.allow_list.contains(&token) {
            return Ok(TokenQuality::Good);
        }

        if self.deny_list.contains(&token) {
            return Ok(TokenQuality::Bad {
                reason: "deny listed".to_string(),
            });
        }

        match &self.strategy {
            UnknownTokenStrategy::Allow => Ok(TokenQuality::Good),
            UnknownTokenStrategy::Deny => Ok(TokenQuality::Bad {
                reason: "default deny".to_string(),
            }),
            UnknownTokenStrategy::Forward(inner) => inner.detect(token).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bad_token::MockBadTokenDetecting;
    use futures::FutureExt;

    #[test]
    fn uses_lists() {
        // Would panic if used.
        let inner = MockBadTokenDetecting::new();
        let detector = ListBasedDetector {
            allow_list: vec![H160::from_low_u64_le(0)],
            deny_list: vec![H160::from_low_u64_le(1)],
            strategy: UnknownTokenStrategy::Forward(Box::new(inner)),
        };

        let result = detector
            .detect(H160::from_low_u64_le(0))
            .now_or_never()
            .unwrap();
        assert!(result.unwrap().is_good());

        let result = detector
            .detect(H160::from_low_u64_le(1))
            .now_or_never()
            .unwrap();
        assert!(!result.unwrap().is_good());
    }

    #[test]
    fn not_in_list_default() {
        let detector = ListBasedDetector {
            allow_list: Vec::new(),
            deny_list: Vec::new(),
            strategy: UnknownTokenStrategy::Allow,
        };
        let result = detector
            .detect(H160::from_low_u64_le(0))
            .now_or_never()
            .unwrap();
        assert!(result.unwrap().is_good());

        let detector = ListBasedDetector {
            allow_list: Vec::new(),
            deny_list: Vec::new(),
            strategy: UnknownTokenStrategy::Deny,
        };
        let result = detector
            .detect(H160::from_low_u64_le(0))
            .now_or_never()
            .unwrap();
        assert!(!result.unwrap().is_good());
    }

    #[test]
    fn not_in_list_forwards() {
        let mut inner = MockBadTokenDetecting::new();
        inner
            .expect_detect()
            .times(1)
            .returning(|_| Ok(TokenQuality::Good));

        let detector = ListBasedDetector {
            allow_list: Vec::new(),
            deny_list: Vec::new(),
            strategy: UnknownTokenStrategy::Forward(Box::new(inner)),
        };

        let result = detector
            .detect(H160::from_low_u64_le(0))
            .now_or_never()
            .unwrap();
        assert!(result.unwrap().is_good());
    }
}

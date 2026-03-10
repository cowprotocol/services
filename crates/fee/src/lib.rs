use {
    crate::factor::FeeFactor,
    alloy_primitives::Address,
    anyhow::{Context, Result, ensure},
    std::{collections::HashSet, str::FromStr},
};

pub mod factor;
pub mod parameters;

/// Helper type for parsing token bucket fee overrides from strings
#[derive(Debug, Clone)]
pub struct TokenBucketFeeOverride {
    pub tokens: HashSet<Address>,
    pub factor: FeeFactor,
}

impl FromStr for TokenBucketFeeOverride {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (factor_str, tokens_str) = s.split_once(':').with_context(|| {
            format!(
                "invalid bucket override format: expected 'factor:token1;token2;...', got '{}'",
                s
            )
        })?;
        let factor = factor_str
            .parse::<f64>()
            .context("failed to parse fee factor")?
            .try_into()
            .context("fee factor out of range")?;
        let tokens: HashSet<Address> = tokens_str
            .split(';')
            .map(|token| {
                token
                    .parse::<Address>()
                    .with_context(|| format!("failed to parse token address '{}'", token))
            })
            .collect::<Result<HashSet<Address>>>()?;

        ensure!(
            tokens.len() >= 2,
            "bucket override must contain at least 2 tokens, got {}",
            tokens.len()
        );

        Ok(TokenBucketFeeOverride { tokens, factor })
    }
}

#[cfg(test)]
mod tests {
    use {crate::TokenBucketFeeOverride, alloy_primitives::address, std::str::FromStr};

    #[test]
    fn parse_token_bucket_fee_override() {
        // Valid inputs with 2 tokens (minimum required)
        let valid_two_tokens = "0.5:0x0000000000000000000000000000000000000001;\
                                0x0000000000000000000000000000000000000002";
        let result = TokenBucketFeeOverride::from_str(valid_two_tokens).unwrap();
        assert_eq!(result.factor.get(), 0.5);
        assert_eq!(result.tokens.len(), 2);
        assert!(
            result
                .tokens
                .contains(&address!("0000000000000000000000000000000000000001"))
        );
        assert!(
            result
                .tokens
                .contains(&address!("0000000000000000000000000000000000000002"))
        );

        // Valid inputs with 3 tokens
        let valid_three_tokens = "0.123:0x0000000000000000000000000000000000000001;\
                                  0x0000000000000000000000000000000000000002;\
                                  0x0000000000000000000000000000000000000003";
        let result = TokenBucketFeeOverride::from_str(valid_three_tokens).unwrap();
        assert_eq!(result.factor.get(), 0.123);
        assert_eq!(result.tokens.len(), 3);
        // Invalid: only 1 token (need at least 2)
        assert!(
            TokenBucketFeeOverride::from_str("0.5:0x0000000000000000000000000000000000000001")
                .is_err()
        );
        // Invalid: wrong format (no colon)
        assert!(
            TokenBucketFeeOverride::from_str("0.5,0x0000000000000000000000000000000000000001")
                .is_err()
        );
        // Invalid: too many parts
        assert!(
            TokenBucketFeeOverride::from_str(
                "0.5:0x0000000000000000000000000000000000000001:extra"
            )
            .is_err()
        );
        // Invalid: fee factor out of range
        assert!(
            TokenBucketFeeOverride::from_str("1.5:0x0000000000000000000000000000000000000001")
                .is_err()
        );
        assert!(
            TokenBucketFeeOverride::from_str("-0.1:0x0000000000000000000000000000000000000001")
                .is_err()
        );
        // Invalid: not a number for fee factor
        assert!(
            TokenBucketFeeOverride::from_str("abc:0x0000000000000000000000000000000000000001")
                .is_err()
        );
        // Invalid: bad token address
        assert!(
            TokenBucketFeeOverride::from_str(
                "0.5:notanaddress,0x0000000000000000000000000000000000000002"
            )
            .is_err()
        );
    }
}

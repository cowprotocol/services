//! Classification of order-simulation revert reasons.
//!
//! Used to decide whether to accept or reject an order whose simulation
//! returned a revert. Funding-class reverts mean the order is not currently
//! fillable due to allowance or balance state but may become fillable later,
//! and are accepted because CoW intentionally allows fund-later orders.
//! Everything else lands in Other, which is rejected and alerted on so the
//! funding pattern set can grow.
//!
//! We deliberately maintain only the funding allowlist. Enumerating structural
//! reverts would be whack-a-mole because every new token and wrapper introduces
//! its own variant. Defaulting to Other is honest about that.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevertClass {
    Funding,
    Other,
}

impl RevertClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            RevertClass::Funding => "funding",
            RevertClass::Other => "other",
        }
    }
}

/// Classifies a raw revert reason as returned by the order simulator
/// (the `reason` field of `OrderSimulationError::Reverted`).
///
/// Inputs look like one of:
/// - `server returned an error response: error code 3: execution reverted:
///   <REASON>, data: "0x..."`
/// - `server returned an error response: error code 3: execution reverted,
///   data: "0x..."`
/// - `server returned an error response: error code 3: execution reverted`
pub fn classify(reason: &str) -> RevertClass {
    if FUNDING_TEXT_PATTERNS.iter().any(|p| reason.contains(p)) {
        return RevertClass::Funding;
    }
    if let Some(selector) = extract_selector(reason)
        && FUNDING_SELECTORS.contains(&selector.as_str())
    {
        return RevertClass::Funding;
    }
    RevertClass::Other
}

// Substring patterns matched against the raw RPC error string. Patterns are
// intentionally written without ERC20:/BEP20:/Dai: prefixes so they catch
// every token-specific variant we have observed in production.
const FUNDING_TEXT_PATTERNS: &[&str] = &[
    "transfer amount exceeds balance",
    "transfer amount exceeds allowance",
    "transfer amount exceeds spender allowance",
    "insufficient allowance",
    "insufficient-balance",
    "insufficient-allowance",
    "BALANCE_EXCEEDED",
    "ALLOWANCE_EXCEEDED",
    "TRANSFER_AMOUNT_EXCEEDS_BALANCE",
    "Insufficient transferable balance",
    "available balance has been locked",
    "TRANSFER_FROM_FAILED",
    "GPv2: failed transfer",
    "SUDC: Tokens locked",
    "Trading is Paused",
];

// 4-byte custom-error selectors, lowercase.
const FUNDING_SELECTORS: &[&str] = &[
    // ERC20InsufficientBalance(address,uint256,uint256) - OpenZeppelin ERC20 v5
    "0xe450d38c",
];

/// Returns the first `0x` + 8 hex chars found in the reason, lowercased.
fn extract_selector(reason: &str) -> Option<String> {
    let bytes = reason.as_bytes();
    let mut i = 0;
    while i + 10 <= bytes.len() {
        if bytes[i] == b'0'
            && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X')
            && bytes[i + 2..i + 10].iter().all(|b| b.is_ascii_hexdigit())
        {
            return Some(reason[i..i + 10].to_ascii_lowercase());
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixtures derived from real production revert strings across all chains
    /// over the past 7 days.
    #[test]
    fn funding_string_reverts() {
        let cases = [
            r#"server returned an error response: error code 3: execution reverted: ERC20: insufficient allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: ERC20: transfer amount exceeds allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: ERC20: transfer amount exceeds balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: BEP20: transfer amount exceeds allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: BEP20: transfer amount exceeds balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Gear::_transferTokens: transfer amount exceeds balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Gear::transferFrom: transfer amount exceeds spender allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Ondo::transferFrom: transfer amount exceeds spender allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: BALANCE_EXCEEDED, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: ALLOWANCE_EXCEEDED, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: TRANSFER_AMOUNT_EXCEEDS_BALANCE, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Dai/insufficient-balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Dai/insufficient-allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Usds/insufficient-balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: SUsds/insufficient-allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Insufficient transferable balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: some available balance has been locked and will be unlocked gradually after unlock time, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: TransferHelper: TRANSFER_FROM_FAILED, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: GPv2: failed transfer, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: SUDC: Tokens locked, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Trading is Paused, data: "0x08c379a0...""#,
        ];
        for case in cases {
            assert_eq!(classify(case), RevertClass::Funding, "case: {case}");
        }
    }

    #[test]
    fn funding_custom_error_selector() {
        // OZ v5 ERC20InsufficientBalance(address,uint256,uint256)
        let reason = r#"server returned an error response: error code 3: execution reverted, data: "0xe450d38c000000000000000000000000aa4ae04691e78dbf8c2f6e6db627d0d2ab0a2914000000000000000000000000000000000000000000000000000000275deeb846000000000000000000000000000000000000000000000051771e4b0c7dac282b""#;
        assert_eq!(classify(reason), RevertClass::Funding);
    }

    /// Anything not in the funding allowlist defaults to Other, whether it is
    /// a revert we are confident is structural, a token-specific quirk, a
    /// bare revert, or a custom-error selector we have not classified.
    #[test]
    fn other_for_non_funding() {
        let cases = [
            // CoW protocol-level reverts.
            r#"server returned an error response: error code 3: execution reverted: GPv2: not a contract, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: GPv2: order expired, data: "0x08c379a0...""#,
            // Solidity panics.
            r#"server returned an error response: error code 3: execution reverted: panic: arithmetic underflow or overflow (0x11), data: "0x4e487b710000000000000000000000000000000000000000000000000000000000000011""#,
            r#"server returned an error response: error code 3: execution reverted: SafeMath: subtraction overflow, data: "0x08c379a0...""#,
            // Token-specific reverts that imply the order configuration is invalid.
            r#"server returned an error response: error code 3: execution reverted: TRANSFER_TO_SELF, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: you cannot transfer to yourself, data: "0x08c379a0...""#,
            // Bare revert with no data.
            "server returned an error response: error code 3: execution reverted",
            // Undecoded custom-error selectors observed in production.
            r#"server returned an error response: error code 3: execution reverted, data: "0xfb8f41b2000000000000000000000000c92e8bdf79f0507f65a392b0ab4667716bfe0110000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004128c7b75d12538ce0""#,
            r#"server returned an error response: error code 3: execution reverted, data: "0xec442f050000000000000000000000000000000000000000000000000000000000000000""#,
            // Genuinely unseen patterns.
            r#"server returned an error response: error code 3: execution reverted: SomeNewWeirdError: explanation here, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted, data: "0xdeadbeef000000000000000000000000aa4ae04691e78dbf""#,
        ];
        for case in cases {
            assert_eq!(classify(case), RevertClass::Other, "case: {case}");
        }
    }

    #[test]
    fn selector_extraction_handles_mixed_case() {
        let reason = r#"data: "0xE450D38C0000...""#;
        assert_eq!(extract_selector(reason).as_deref(), Some("0xe450d38c"));
    }

    #[test]
    fn class_strings_match_metric_label_values() {
        assert_eq!(RevertClass::Funding.as_str(), "funding");
        assert_eq!(RevertClass::Other.as_str(), "other");
    }
}

//! Classification of order-simulation revert reasons.
//!
//! Funding-class reverts (allowance or balance shortfalls hit while simulating
//! the full sell amount) are accepted. Funding is already gated upstream by
//! the cheap allowance check in `ensure_token_is_transferable`, which
//! intentionally permits partial-funding orders so users can submit before
//! they are fully funded. A funding-class revert from the deeper simulation
//! is therefore either a permitted partial-funding order (fillable once the
//! user funds the rest), or an artifact of a state override the simulator
//! could not compute (e.g. stETH as the buy token). The deeper simulation's
//! job is structural validation, not re-litigating funding.
//!
//! Everything else lands in Other, which is rejected and alerted on so new
//! funding patterns can be added when discovered.

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

    #[test]
    fn funding_string_reverts() {
        let cases = [
            r#"execution reverted: ERC20: insufficient allowance, data: "0x...""#,
            r#"execution reverted: BEP20: transfer amount exceeds balance, data: "0x...""#,
            r#"execution reverted: BALANCE_EXCEEDED, data: "0x...""#,
            r#"execution reverted: Dai/insufficient-allowance, data: "0x...""#,
            r#"execution reverted: TransferHelper: TRANSFER_FROM_FAILED, data: "0x...""#,
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

    #[test]
    fn other_for_non_funding() {
        let cases = [
            r#"execution reverted: GPv2: order expired, data: "0x...""#,
            r#"execution reverted: panic: arithmetic underflow or overflow (0x11), data: "0x...""#,
            "execution reverted",
            r#"execution reverted, data: "0xdeadbeef00...""#,
            r#"execution reverted: SomeNewError: details, data: "0x...""#,
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

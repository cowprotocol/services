//! Classification of order-simulation revert reasons.
//!
//! Used to decide whether to accept or reject an order whose simulation
//! returned a revert. Funding-class reverts mean the order is not currently
//! fillable due to allowance or balance state but may become fillable later,
//! and are accepted because CoW intentionally allows fund-later orders.
//! Structural-class reverts mean the order will never settle regardless of
//! funding (panics, broken hooks, expired, bare reverts), and are rejected.
//! Unknown is for reasons not yet classified, treated as accept-and-alert so
//! the pattern set can grow.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevertClass {
    Funding,
    Structural,
    Unknown,
}

impl RevertClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            RevertClass::Funding => "funding",
            RevertClass::Structural => "structural",
            RevertClass::Unknown => "unknown",
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
    if STRUCTURAL_TEXT_PATTERNS.iter().any(|p| reason.contains(p)) {
        return RevertClass::Structural;
    }
    if let Some(selector) = extract_selector(reason) {
        if FUNDING_SELECTORS.contains(&selector.as_str()) {
            return RevertClass::Funding;
        }
        if STRUCTURAL_SELECTORS.contains(&selector.as_str()) {
            return RevertClass::Structural;
        }
    }
    // Bare revert: contract ran `revert()` with no reason and no data. In our
    // logs these correlate with flashloan and hook wrapper failures.
    if reason.trim_end().ends_with("execution reverted") {
        return RevertClass::Structural;
    }
    RevertClass::Unknown
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

const STRUCTURAL_TEXT_PATTERNS: &[&str] = &[
    "GPv2: not a contract",
    "GPv2: order expired",
    "arithmetic underflow or overflow",
    "SafeMath: subtraction overflow",
    "TRANSFER_TO_SELF",
    "you cannot transfer to yourself",
    "from and to can not be be the same",
    "sender and recipient are the same",
];

// 4-byte custom-error selectors, lowercase.
const FUNDING_SELECTORS: &[&str] = &[
    // ERC20InsufficientBalance(address,uint256,uint256) - OpenZeppelin ERC20 v5
    "0xe450d38c",
];

const STRUCTURAL_SELECTORS: &[&str] = &[
    // Panic(uint256) - Solidity built-in
    "0x4e487b71",
    // ERC20InvalidReceiver(address) - OpenZeppelin ERC20 v5
    "0xec442f05",
    // Observed in flashloan and wrapper failures across chains.
    "0xfb8f41b2",
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

    #[test]
    fn structural_string_reverts() {
        let cases = [
            r#"server returned an error response: error code 3: execution reverted: GPv2: not a contract, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: GPv2: order expired, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: panic: arithmetic underflow or overflow (0x11), data: "0x4e487b710000000000000000000000000000000000000000000000000000000000000011""#,
            r#"server returned an error response: error code 3: execution reverted: SafeMath: subtraction overflow, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: TRANSFER_TO_SELF, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: you cannot transfer to yourself, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: from and to can not be be the same , data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: sender and recipient are the same (_from = _to), data: "0x08c379a0...""#,
        ];
        for case in cases {
            assert_eq!(classify(case), RevertClass::Structural, "case: {case}");
        }
    }

    #[test]
    fn structural_custom_error_selectors() {
        let panic = r#"server returned an error response: error code 3: execution reverted, data: "0x4e487b710000000000000000000000000000000000000000000000000000000000000011""#;
        assert_eq!(classify(panic), RevertClass::Structural);

        let invalid_receiver = r#"server returned an error response: error code 3: execution reverted, data: "0xec442f050000000000000000000000000000000000000000000000000000000000000000""#;
        assert_eq!(classify(invalid_receiver), RevertClass::Structural);

        let wrapper = r#"server returned an error response: error code 3: execution reverted, data: "0xfb8f41b2000000000000000000000000c92e8bdf79f0507f65a392b0ab4667716bfe0110000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004128c7b75d12538ce0""#;
        assert_eq!(classify(wrapper), RevertClass::Structural);
    }

    #[test]
    fn bare_revert_is_structural() {
        let bare = "server returned an error response: error code 3: execution reverted";
        assert_eq!(classify(bare), RevertClass::Structural);
        // Trailing whitespace should not defeat detection.
        let bare_ws = "server returned an error response: error code 3: execution reverted \n";
        assert_eq!(classify(bare_ws), RevertClass::Structural);
    }

    #[test]
    fn unknown_for_unrecognized_selector() {
        let reason = r#"server returned an error response: error code 3: execution reverted, data: "0xdeadbeef000000000000000000000000aa4ae04691e78dbf""#;
        assert_eq!(classify(reason), RevertClass::Unknown);
    }

    #[test]
    fn unknown_for_unrecognized_text() {
        let cases = [
            // Genuinely new pattern we haven't seen.
            r#"server returned an error response: error code 3: execution reverted: SomeNewWeirdError: explanation here, data: "0x08c379a0...""#,
            // Per-token caps are intentionally left Unknown until we decide a policy.
            r#"server returned an error response: error code 3: execution reverted: Transfer amount exceeds the maxTxAmount., data: "0x08c379a0...""#,
        ];
        for case in cases {
            assert_eq!(classify(case), RevertClass::Unknown, "case: {case}");
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
        assert_eq!(RevertClass::Structural.as_str(), "structural");
        assert_eq!(RevertClass::Unknown.as_str(), "unknown");
    }
}

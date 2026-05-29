//! Classification of order-simulation revert reasons.
//!
//! Used by enforce-mode to decide whether to accept or reject an order whose
//! shadow simulation returned a revert. Funding-class reverts mean the order
//! is not currently fillable due to allowance/balance/expiry state but may
//! become fillable later, and are accepted because CoW intentionally allows
//! fund-later orders. Structural-class reverts mean the order will never
//! settle regardless of funding (panics, broken hooks, bare reverts), and are
//! rejected. Unknown is for reasons not yet classified, treated as accept and
//! alert so the pattern set can grow.

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

/// Classifies a raw revert reason as returned by the shadow order simulator
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
    // shadow logs these correlate with flashloan/hook wrapper failures.
    if reason.trim_end().ends_with("execution reverted") {
        return RevertClass::Structural;
    }
    RevertClass::Unknown
}

const FUNDING_TEXT_PATTERNS: &[&str] = &[
    "ERC20: transfer amount exceeds balance",
    "ERC20: transfer amount exceeds allowance",
    "ERC20: insufficient allowance",
    "BEP20: transfer amount exceeds balance",
    "BEP20: transfer amount exceeds allowance",
    "BALANCE_EXCEEDED",
    "transfer amount exceeds spender allowance",
    "insufficient-balance",
    "insufficient-allowance",
    "GPv2: order expired",
];

const STRUCTURAL_TEXT_PATTERNS: &[&str] =
    &["GPv2: not a contract", "arithmetic underflow or overflow"];

// 4-byte custom-error selectors, lowercase.
const FUNDING_SELECTORS: &[&str] = &[
    // ERC20InsufficientBalance(address,uint256,uint256) - OpenZeppelin ERC20 v5
    "0xe450d38c",
];

const STRUCTURAL_SELECTORS: &[&str] = &[
    // Panic(uint256) - Solidity built-in
    "0x4e487b71",
    // Observed in flashloan/wrapper failures in our shadow logs across chains.
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

    #[test]
    fn funding_string_reverts() {
        let cases = [
            r#"server returned an error response: error code 3: execution reverted: ERC20: insufficient allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: ERC20: transfer amount exceeds allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: ERC20: transfer amount exceeds balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: BEP20: transfer amount exceeds allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: BEP20: transfer amount exceeds balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: BALANCE_EXCEEDED, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: GPv2: order expired, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: SUsds/insufficient-allowance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Usds/insufficient-balance, data: "0x08c379a0...""#,
            r#"server returned an error response: error code 3: execution reverted: Ondo::transferFrom: transfer amount exceeds spender allowance, data: "0x08c379a0...""#,
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
            r#"server returned an error response: error code 3: execution reverted: panic: arithmetic underflow or overflow (0x11), data: "0x4e487b710000000000000000000000000000000000000000000000000000000000000011""#,
        ];
        for case in cases {
            assert_eq!(classify(case), RevertClass::Structural, "case: {case}");
        }
    }

    #[test]
    fn structural_custom_error_selectors() {
        let panic = r#"server returned an error response: error code 3: execution reverted, data: "0x4e487b710000000000000000000000000000000000000000000000000000000000000011""#;
        assert_eq!(classify(panic), RevertClass::Structural);

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
        let reason = r#"server returned an error response: error code 3: execution reverted: SomeNewWeirdError: explanation here, data: "0x08c379a0...""#;
        assert_eq!(classify(reason), RevertClass::Unknown);
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

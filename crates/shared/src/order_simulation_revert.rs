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

use {regex::Regex, std::sync::OnceLock};

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
    "SUDC: Tokens locked",
];

// 4-byte custom-error selectors, lowercase.
const FUNDING_SELECTORS: &[&str] = &[
    // ERC20InsufficientBalance(address,uint256,uint256) - OpenZeppelin ERC20 v5
    "0xe450d38c",
];

/// Returns the first ABI-encoded 4-byte selector found in the reason,
/// lowercased. A valid selector + args encoding has hex length `8 + 64*N`
/// (selector plus N 32-byte words). The length filter excludes 40-char
/// addresses and 64-char hashes that may appear in the reason text alongside
/// the actual error data.
fn extract_selector(reason: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"0[xX][0-9a-fA-F]+").unwrap());
    re.find_iter(reason)
        .find(|m| (m.len() - 2) % 64 == 8)
        .map(|m| m.as_str()[..10].to_ascii_lowercase())
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
            r#"execution reverted: Insufficient transferable balance, data: "0x...""#,
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
            r#"execution reverted: Trading is Paused, data: "0x...""#,
            r#"execution reverted: GPv2: failed transfer, data: "0x...""#,
            r#"execution reverted: TransferHelper: TRANSFER_FROM_FAILED, data: "0x...""#,
            "execution reverted",
            r#"execution reverted, data: "0xdeadbeef00...""#,
            r#"execution reverted: SomeNewError: details, data: "0x...""#,
        ];
        for case in cases {
            assert_eq!(classify(case), RevertClass::Other, "case: {case}");
        }
    }

    #[test]
    fn selector_extraction_handles_bare_selector_reason() {
        // Reason can be any length, including just the selector with nothing
        // around it (no args, no prefix text, no data field).
        assert_eq!(
            extract_selector("0x4e487b71").as_deref(),
            Some("0x4e487b71")
        );
        // Bare selector wrapped in classifier returns the right class.
        assert_eq!(classify("0x4e487b71"), RevertClass::Other);
        // Too-short hex (not a valid selector) returns None.
        assert_eq!(extract_selector("0x123").as_deref(), None);
        assert_eq!(extract_selector("0x").as_deref(), None);
    }

    #[test]
    fn selector_extraction_handles_mixed_case() {
        // 200 hex chars total = selector + 3 32-byte words = OZ v5
        // ERC20InsufficientBalance(address,uint256,uint256), upper-case to
        // confirm normalization.
        let reason = "0xE450D38C000000000000000000000000AA4AE04691E78DBF8C2F6E6DB627D0D2AB0A2914000000000000000000000000000000000000000000000000000000275DEEB846000000000000000000000000000000000000000000000051771E4B0C7DAC282B";
        assert_eq!(extract_selector(reason).as_deref(), Some("0xe450d38c"));
    }

    #[test]
    fn selector_extraction_ignores_addresses_and_hashes() {
        // Reason embeds an address (40 hex) and a hash (64 hex) before the
        // actual selector + args inside the data field. The length filter
        // skips the first two and lands on the real selector.
        let reason = r#"execution reverted: AccessControl: account 0x1234567890abcdef1234567890abcdef12345678 missing role 0xfedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210, data: "0xe450d38c000000000000000000000000aa4ae04691e78dbf8c2f6e6db627d0d2ab0a2914000000000000000000000000000000000000000000000000000000275deeb846000000000000000000000000000000000000000000000051771e4b0c7dac282b""#;
        assert_eq!(extract_selector(reason).as_deref(), Some("0xe450d38c"));
        assert_eq!(classify(reason), RevertClass::Funding);
    }

    #[test]
    fn class_strings_match_metric_label_values() {
        assert_eq!(RevertClass::Funding.as_str(), "funding");
        assert_eq!(RevertClass::Other.as_str(), "other");
    }
}

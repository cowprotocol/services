/// Parses a conventional feature-flag environment value.
///
/// Disabled values: `0`, `false`, `off` (case-insensitive).
/// Any other present value is treated as enabled.
pub fn flag_enabled(value: Option<&str>, default: bool) -> bool {
    value
        .map(|value| !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "off"))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::flag_enabled;

    #[test]
    fn parses_disabled_values_case_insensitively() {
        assert!(!flag_enabled(Some("0"), true));
        assert!(!flag_enabled(Some("false"), true));
        assert!(!flag_enabled(Some("OFF"), true));
    }

    #[test]
    fn parses_enabled_values_and_default() {
        assert!(flag_enabled(Some("1"), false));
        assert!(flag_enabled(Some("on"), false));
        assert!(flag_enabled(None, true));
        assert!(!flag_enabled(None, false));
    }
}

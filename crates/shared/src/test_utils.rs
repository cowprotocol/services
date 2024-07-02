use {anyhow::anyhow, std::collections::HashSet};
/// Asserts that two JSON values are equal, excluding specified paths.
///
/// This macro is used to compare two JSON values for equality while ignoring
/// certain paths in the JSON structure. The paths to be ignored are specified
/// as a list of dot-separated strings. If the two JSON values are not equal
/// (excluding the ignored paths), the macro will panic with a detailed error
/// message indicating the location of the discrepancy.
///
/// # Arguments
///
/// * `$actual` - The actual JSON value obtained in a test.
/// * `$expected` - The expected JSON value for comparison.
/// * `$exclude_paths` - An array of dot-separated strings specifying the paths
///   to be ignored during comparison.
///
/// # Panics
///
/// The macro panics if the actual and expected JSON values are not equal,
/// excluding the ignored paths.
///
/// # Examples
///
/// ```
/// let actual = serde_json::json!({"user": {"id": 1, "name": "Alice", "email": "alice@example.com"}});
/// let expected = serde_json::json!({"user": {"id": 1, "name": "Alice", "email": "bob@example.com"}});
/// shared::assert_json_matches_excluding!(actual, expected, ["user.email"]);
/// ```
#[macro_export]
macro_rules! assert_json_matches_excluding {
    ($actual:expr, $expected:expr, $exclude_paths:expr) => {{
        let exclude_paths = $crate::test_utils::parse_field_paths(&$exclude_paths);
        let result =
            $crate::test_utils::json_matches_excluding(&$actual, &$expected, &exclude_paths);
        if let Err(e) = result {
            panic!(
                "JSON did not match with the exclusion of specified paths. Error: {}\nActual \
                 JSON: {}\nExpected JSON: {}",
                e,
                serde_json::to_string_pretty(&$actual).unwrap(),
                serde_json::to_string_pretty(&$expected).unwrap()
            );
        }
    }};
}

/// Asserts that two JSON values are exactly equal.
///
/// This macro is used to compare two JSON values for strict equality in a
/// testing context. If the two JSON values are not equal, the macro will panic
/// with a detailed error message indicating the location of the discrepancy.
///
/// # Arguments
///
/// * `$actual` - The actual JSON value obtained in a test, typically the output
///   of the code being tested.
/// * `$expected` - The expected JSON value for comparison, typically the
///   expected outcome in a test scenario.
///
/// # Panics
///
/// The macro panics if the actual and expected JSON values are not exactly
/// equal.
///
/// # Examples
///
/// ```
/// use shared::assert_json_matches;
///
/// let actual = serde_json::json!({"user": {"id": 1, "name": "Alice"}});
/// let expected = serde_json::json!({"user": {"id": 1, "name": "Alice"}});
/// assert_json_matches!(actual, expected);
/// ```
///
/// In this example, the `assert_json_matches!` macro is used to assert that the
/// `actual` JSON object is exactly the same as the `expected` JSON object. If
/// there are any differences between the two, the test will fail with a panic.
#[macro_export]
macro_rules! assert_json_matches {
    ($actual:expr, $expected:expr) => {{
        let result = $crate::test_utils::json_matches_excluding(
            &$actual,
            &$expected,
            &std::collections::HashSet::new(),
        );
        if let Err(e) = result {
            panic!(
                "JSON did not match. Error: {}\nActual JSON: {}\nExpected JSON: {}",
                e,
                serde_json::to_string_pretty(&$actual).unwrap(),
                serde_json::to_string_pretty(&$expected).unwrap()
            );
        }
    }};
}

/// Parses dot-separated field paths into a set of paths.
pub fn parse_field_paths(paths: &[&str]) -> HashSet<Vec<String>> {
    paths
        .iter()
        .map(|path| path.split('.').map(String::from).collect())
        .collect()
}

/// Recursively compares two JSON values, excluding specified paths, and returns
/// detailed errors using anyhow.
pub fn json_matches_excluding(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    exclude_paths: &HashSet<Vec<String>>,
) -> anyhow::Result<()> {
    /// A helper function that recursively compares two JSON values using
    /// Depth-First Search (DFS) traversal. It utilizes a `current_path`
    /// accumulator to maintain the current path within the JSON structure,
    /// which is then compared against the `exclude_paths` parameter.
    /// During the backtracking process, the function updates the `current_path`
    /// accumulator to reflect the current position in the JSON structure.
    fn compare_jsons(
        actual: &serde_json::Value,
        expected: &serde_json::Value,
        exclude_paths: &HashSet<Vec<String>>,
        current_path: &mut Vec<String>,
    ) -> anyhow::Result<()> {
        match (actual, expected) {
            (serde_json::Value::Object(map_a), serde_json::Value::Object(map_b)) => {
                let keys: HashSet<_> = map_a.keys().chain(map_b.keys()).cloned().collect();
                for key in keys {
                    current_path.push(key.clone());

                    if exclude_paths.contains(current_path) {
                        current_path.pop();
                        continue;
                    }

                    match (map_a.get(&key), map_b.get(&key)) {
                        (Some(value_a), Some(value_b)) => {
                            if let Err(e) =
                                compare_jsons(value_a, value_b, exclude_paths, current_path)
                            {
                                current_path.pop();
                                return Err(e);
                            }
                        }
                        (None, Some(_)) => {
                            let error_msg = format!(
                                "Key missing in actual JSON at {}",
                                current_path.join("."),
                            );
                            current_path.pop();
                            return Err(anyhow!(error_msg));
                        }
                        (Some(_), None) => {
                            let error_msg = format!(
                                "Key missing in expected JSON at {}",
                                current_path.join("."),
                            );
                            current_path.pop();
                            return Err(anyhow!(error_msg));
                        }
                        (None, None) => unreachable!(),
                    }

                    current_path.pop();
                }
                Ok(())
            }
            _ => {
                if actual != expected {
                    Err(anyhow!(
                        "Mismatch at {}: {:?} != {:?}",
                        current_path.join("."),
                        actual,
                        expected
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    let mut current_path = vec![];
    compare_jsons(actual, expected, exclude_paths, &mut current_path)
}

#[cfg(test)]
mod tests {
    use {super::*, maplit::hashset, serde_json::json};

    #[test]
    fn test_parse_field_paths() {
        let paths = ["user.profile.name", "user.settings"];
        let parsed_paths = parse_field_paths(&paths);
        let expected_paths: HashSet<Vec<String>> = hashset! {
            vec!["user".to_string(), "profile".to_string(), "name".to_string()],
            vec!["user".to_string(), "settings".to_string()],
        };
        assert_eq!(parsed_paths, expected_paths)
    }

    #[test]
    fn test_json_matches_excluding_no_exclusions() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice"
                }
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice"
                }
            }
        });
        assert_json_matches!(json_a, json_b)
    }

    #[test]
    fn test_json_matches_excluding_with_exclusions() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2021-01-01T12:00:00Z"
                },
                "enabled": true,
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2022-01-01T12:00:00Z"
                },
                "enabled": false,
            }
        });
        assert_json_matches_excluding!(json_a, json_b, ["user.profile.timestamp", "user.enabled"])
    }

    #[test]
    #[should_panic(
        expected = r#"JSON did not match. Error: Mismatch at user.profile.name: String("Alice") != String("Bob")
Actual JSON: {
  "user": {
    "id": 123,
    "profile": {
      "name": "Alice",
      "timestamp": "2021-01-01T12:00:00Z"
    }
  }
}
Expected JSON: {
  "user": {
    "id": 123,
    "profile": {
      "name": "Bob",
      "timestamp": "2021-01-01T12:00:00Z"
    }
  }
}"#
    )]
    fn test_json_matches_excluding_failure() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Bob",
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        assert_json_matches!(json_a, json_b)
    }

    #[test]
    #[should_panic(
        expected = r#"JSON did not match. Error: Key missing in expected JSON at user.profile.name
Actual JSON: {
  "user": {
    "id": 123,
    "profile": {
      "name": "Alice",
      "timestamp": "2021-01-01T12:00:00Z"
    }
  }
}
Expected JSON: {
  "user": {
    "id": 123,
    "profile": {
      "timestamp": "2021-01-01T12:00:00Z"
    }
  }
}"#
    )]
    fn test_json_matches_excluding_key_is_missing() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        assert_json_matches!(json_a, json_b)
    }

    #[test]
    #[should_panic(
        expected = r#"JSON did not match. Error: Key missing in actual JSON at user.profile.name
Actual JSON: {
  "user": {
    "id": 123,
    "profile": {
      "timestamp": "2021-01-01T12:00:00Z"
    }
  }
}
Expected JSON: {
  "user": {
    "id": 123,
    "profile": {
      "name": "Alice",
      "timestamp": "2021-01-01T12:00:00Z"
    }
  }
}"#
    )]
    fn test_json_matches_excluding_key_is_missing_reversed() {
        let json_a = json!({
            "user": {
                "id": 123,
                "profile": {
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        let json_b = json!({
            "user": {
                "id": 123,
                "profile": {
                    "name": "Alice",
                    "timestamp": "2021-01-01T12:00:00Z"
                }
            }
        });
        assert_json_matches!(json_a, json_b)
    }
}

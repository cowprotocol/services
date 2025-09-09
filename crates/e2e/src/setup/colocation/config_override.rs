use {
    anyhow::{Context, Result},
    toml::{Table, Value},
};

/// Simple TOML config builder that supports raw TOML merging
pub struct TomlConfigBuilder {
    base_template: String,
}

impl TomlConfigBuilder {
    pub fn new(base_template: String) -> Self {
        Self { base_template }
    }

    /// Build config with optional raw TOML override
    pub fn build_with_override(&self, raw_override: &str) -> Result<String> {
        // Parse base config
        let mut base: Table =
            toml::from_str(&self.base_template).context("Failed to parse base TOML config")?;

        // Parse override config
        let override_table: Table =
            toml::from_str(raw_override).context("Failed to parse override TOML")?;

        // Merge override into base
        merge_tables(&mut base, &override_table);

        // Serialize back to string
        toml::to_string_pretty(&base).context("Failed to serialize TOML config")
    }
}

/// Recursively merge two TOML tables
fn merge_tables(base: &mut Table, overlay: &Table) {
    for (key, value) in overlay {
        match (base.get_mut(key), value) {
            (Some(Value::Table(base_table)), Value::Table(overlay_table)) => {
                // Both are tables, merge recursively
                merge_tables(base_table, overlay_table);
            }
            _ => {
                // Replace value entirely (including arrays)
                base.insert(key.clone(), value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_override() {
        let base_config = r#"
orderbook-url = "http://localhost:8080"
"#
        .to_string();

        let raw_override = r#"
orderbook-url = "http://remotehost:8080"
"#;

        let builder = TomlConfigBuilder::new(base_config);
        let result = builder.build_with_override(raw_override).unwrap();

        let parsed: Table = toml::from_str(&result).unwrap();

        // Check that database section was merged
        let orderbook_url = parsed.get("orderbook-url").unwrap().as_str().unwrap();
        assert_eq!(orderbook_url, "http://remotehost:8080");
    }

    #[test]
    fn test_section_override() {
        let base_config = r#"
[database]
host = "localhost"
port = 5432

[logging]
level = "info"
"#
        .to_string();

        let raw_override = r#"
[database]
host = "remote-host"
port = 3306
ssl = true

[metrics]
enabled = true
port = 9090
"#;

        let builder = TomlConfigBuilder::new(base_config);
        let result = builder.build_with_override(raw_override).unwrap();

        let parsed: Table = toml::from_str(&result).unwrap();

        // Check that database section was merged
        let database = parsed.get("database").unwrap().as_table().unwrap();
        assert_eq!(
            database.get("host").unwrap().as_str().unwrap(),
            "remote-host"
        );
        assert_eq!(database.get("port").unwrap().as_integer().unwrap(), 3306);
        assert_eq!(database.get("ssl").unwrap().as_bool().unwrap(), true);

        // Check that new metrics section was added
        let metrics = parsed.get("metrics").unwrap().as_table().unwrap();
        assert_eq!(metrics.get("enabled").unwrap().as_bool().unwrap(), true);
        assert_eq!(metrics.get("port").unwrap().as_integer().unwrap(), 9090);

        // Check that logging section remains unchanged
        let logging = parsed.get("logging").unwrap().as_table().unwrap();
        assert_eq!(logging.get("level").unwrap().as_str().unwrap(), "info");
    }

    #[test]
    fn test_array_override() {
        let base_config = r#"
[[servers]]
name = "server1"
port = 8080

[config]
active = true
"#
        .to_string();

        let raw_override = r#"
[[servers]]
name = "new-server1"
port = 9000

[[servers]]
name = "new-server2"
port = 9001
"#;

        let builder = TomlConfigBuilder::new(base_config);
        let result = builder.build_with_override(raw_override).unwrap();

        let parsed: Table = toml::from_str(&result).unwrap();

        // Arrays are replaced entirely, not merged
        let servers = parsed.get("servers").unwrap().as_array().unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(
            servers[0].get("name").unwrap().as_str().unwrap(),
            "new-server1"
        );
        assert_eq!(
            servers[1].get("name").unwrap().as_str().unwrap(),
            "new-server2"
        );

        // Other sections remain
        let config = parsed.get("config").unwrap().as_table().unwrap();
        assert_eq!(config.get("active").unwrap().as_bool().unwrap(), true);
    }
}

use {
    serde::{Deserialize, Deserializer},
    url::Url,
};

/// Deserialize a URL, ensuring its path ends with a trailing `/`.
///
/// This makes `Url::join("solve")` append `solve` rather than replacing the
/// last path segment (e.g. `http://host/api` -> `http://host/api/solve`).
pub fn deserialize_url_with_trailing_slash<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let mut url = Url::deserialize(deserializer)?;
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde::de::value::{Error, StrDeserializer},
    };

    #[test]
    fn appends_trailing_slash_when_missing() {
        let de = StrDeserializer::<Error>::new("http://localhost:8001/api");
        let url: Url = deserialize_url_with_trailing_slash(de).unwrap();
        assert_eq!(url.as_str(), "http://localhost:8001/api/");
        assert_eq!(
            url.join("solve").unwrap().as_str(),
            "http://localhost:8001/api/solve"
        );
    }

    #[test]
    fn keeps_existing_trailing_slash() {
        let de = StrDeserializer::<Error>::new("http://localhost:8001/api/");
        let url: Url = deserialize_url_with_trailing_slash(de).unwrap();
        assert_eq!(url.as_str(), "http://localhost:8001/api/");
    }
}

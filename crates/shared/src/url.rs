use {anyhow::Result, url::Url};

/// Join a path with a URL, ensuring that there is only one slash between them.
/// It doesn't matter if the URL ends with a slash or the path starts with one.
pub fn join(url: &Url, mut path: &str) -> Url {
    let mut url = url.to_string();
    while url.ends_with('/') {
        url.pop();
    }
    while path.starts_with('/') {
        path = &path[1..]
    }
    Url::parse(&format!("{url}/{path}")).unwrap()
}

/// Replaces the last path segment of the URL with the new endpoint.
/// Returns an error if `url` has an empty path meaning nothing can be replaced.
pub fn replace_endpoint(mut url: Url, new_endpoint: &str) -> Result<Url> {
    if url.path() == "/" || url.path().is_empty() {
        anyhow::bail!("url has no endpoint to replace: {url}");
    }
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("cannot be a base url"))?
        .pop()
        .push(new_endpoint);
    Ok(url)
}

/// Splits a URL right where the path begins into base and endpoint.
/// https://my.solver.xyz/solve/1?param=1#fragment=some
/// becomes
/// (https://my.solver.xyz/, /solve/1?param=1#fragment=some)
/// Path that were split like this can be joined to the original URL using
/// [`join`].
pub fn split_at_path(url: &Url) -> Result<(Url, String)> {
    let base = format!("{}://{}/", url.scheme(), url.authority())
        .parse()
        .expect("stripping off the path is always safe");
    let endpoint = format!(
        "{}{}{}",
        url.path(),
        url.query()
            .map(|params| format!("?{params}"))
            .unwrap_or_default(),
        url.fragment()
            .map(|params| format!("#{params}"))
            .unwrap_or_default(),
    );
    Ok((base, endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that we can split a URL and join it back together without messing
    /// up the URL on the way.
    #[test]
    fn split_and_join() {
        let round_trip = |s: &str| {
            let url = s.parse().unwrap();
            let (base, endpoint) = split_at_path(&url).unwrap();
            assert_eq!(url, join(&base, &endpoint));
        };

        // base + port + path + multiple params + multiple fragments
        round_trip("https://my.solver.xyz:1234/solve/1?param1=1&param2=2#fragment=1&fragment2=2");
        // base + path + multiple params + multiple fragments
        round_trip("https://my.solver.xyz/solve/1?param1=1&param2=2#fragment=1&fragment2=2");
        // base + path + multiple params
        round_trip("https://my.solver.xyz/solve/1?param1=1&param2=2");
        // base + path + multiple fragments
        round_trip("https://my.solver.xyz/solve/1#fragment1=1&fragment2=2");
        // base + path + single param + single fragment
        round_trip("https://my.solver.xyz/solve/1?param=1#fragment1=1");
        // base + path
        round_trip("https://my.solver.xyz/solve/1");
        // base
        round_trip("http://my.solver.xyz");
        // base + multiple params + multiple fragments
        round_trip("https://my.solver.xyz?param1=1&param2=2#fragment=1&fragment2=2");
        // base (with auth) + path
        round_trip("https://user:pass@my.solver.xyz/solve/1");
    }

    #[test]
    fn replaces_endpoints() {
        let url =
            "https://user:pass@my.solver.xyz:1234/solve?param1=1&param2=2#fragment=1&fragment2=2"
                .parse()
                .unwrap();
        let replaced = replace_endpoint(url, "notify");
        let expected =
            "https://user:pass@my.solver.xyz:1234/notify?param1=1&param2=2#fragment=1&fragment2=2"
                .parse()
                .unwrap();
        assert_eq!(replaced.unwrap(), expected);

        let url = "https://user:pass@my.solver.xyz:1234".parse().unwrap();
        let replaced = replace_endpoint(url, "notify");
        assert!(replaced.is_err());
    }
}

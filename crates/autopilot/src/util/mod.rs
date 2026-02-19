use url::Url;

mod bytes;

pub use self::bytes::Bytes;

/// Joins a path with a URL, ensuring that there is only one slash between them.
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

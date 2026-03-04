//! Module with a few functions duplicated from `shared` in order
//! to break cyclical dependencies and break off this crate.

use {
    std::fmt::{Display, Formatter},
    url::Url,
};

pub mod encoding;
pub mod http_client_factory;

/// Join a path with a URL, ensuring that there is only one slash between them.
/// It doesn't matter if the URL ends with a slash or the path starts with one.
pub fn join_url(url: &Url, mut path: &str) -> Url {
    let mut url = url.to_string();
    while url.ends_with('/') {
        url.pop();
    }
    while path.starts_with('/') {
        path = &path[1..]
    }
    Url::parse(&format!("{url}/{path}")).unwrap()
}

/// anyhow errors are not clonable natively. This is a workaround that creates a
/// new anyhow error based on formatting the error with its inner sources
/// without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}

pub fn display_secret_option<T>(
    f: &mut Formatter<'_>,
    name: &str,
    option: Option<&T>,
) -> std::fmt::Result {
    display_option(f, name, &option.as_ref().map(|_| "SECRET"))
}

pub fn display_option(
    f: &mut Formatter<'_>,
    name: &str,
    option: &Option<impl Display>,
) -> std::fmt::Result {
    write!(f, "{name}: ")?;
    match option {
        Some(display) => writeln!(f, "{display}"),
        None => writeln!(f, "None"),
    }
}

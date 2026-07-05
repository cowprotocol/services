use std::sync::LazyLock;

pub fn git_version() -> &'static str {
    static VERSION: LazyLock<String> = LazyLock::new(|| {
        let sha = std::env::var("GIT_SHA").unwrap_or_else(|_| "unknown".into());
        let branch = std::env::var("GIT_BRANCH").unwrap_or_else(|_| "unknown".into());
        format!("{branch}@{sha}")
    });
    &VERSION
}

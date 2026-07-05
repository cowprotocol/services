pub fn git_version() -> String {
    let sha = std::env::var("GIT_SHA").unwrap_or_else(|_| "unknown".into());
    let branch = std::env::var("GIT_BRANCH").unwrap_or_else(|_| "unknown".into());
    format!("{branch}@{sha}")
}

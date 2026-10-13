pub async fn version() -> &'static str {
    observe::version::git_version()
}

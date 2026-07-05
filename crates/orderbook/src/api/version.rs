pub async fn version_handler() -> &'static str {
    observe::version::git_version()
}

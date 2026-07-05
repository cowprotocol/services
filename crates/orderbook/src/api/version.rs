pub async fn version_handler() -> String {
    observe::version::git_version()
}

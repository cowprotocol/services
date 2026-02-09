pub async fn version_handler() -> &'static str {
    env!("VERGEN_GIT_DESCRIBE")
}

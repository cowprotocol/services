/// The deployed build as `<ref>@<sha>`, the same format the EVM orderbook
/// reports, so the frontend's version footer resolves for Solana too.
pub async fn version() -> &'static str {
    observe::version::git_version()
}

fn main() {
    // Make build system aware of custom config flags to avoid clippy warnings
    // tokio_unstable is only used when explicitly compiled with --cfg
    // tokio_unstable (e.g., in the playground environment)
    println!("cargo::rustc-check-cfg=cfg(tokio_unstable)");
}

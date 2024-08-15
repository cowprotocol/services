fn main() {
    // Make build system aware of custom config flags to avoid clippy warnings
    println!("cargo::rustc-check-cfg=cfg(tokio_unstable)");
}

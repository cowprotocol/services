fn main() {
    if let Err(err) = vergen::EmitBuilder::builder().git_sha(true).emit() {
        eprintln!("WARN: vergen failed: {err:?}");
    }
}

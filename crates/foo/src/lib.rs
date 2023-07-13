#[test]
fn foo() {
    println!("{}", env!("VERGEN_GIT_DESCRIBE"));
}

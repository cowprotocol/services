// <crate>/tests signals to Cargo that files inside of it are integration tests.
// Integration tests are compiled into separate binaries which is slow. To avoid
// this we create one integration test here and in this test we include all the
// tests we want to run.

#[macro_use]
pub mod setup;
pub mod local_node;

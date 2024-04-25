
/// The type of strategy used to encode the solution.
#[derive(Debug, Copy, Clone)]
pub enum Strategy {
    /// Use logic from the legacy solver crate
    Boundary,
    /// Use logic from this module for encoding
    Domain,
}

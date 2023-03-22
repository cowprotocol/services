pub mod file;

pub struct Config {
    pub zeroex: crate::infra::dex::zeroex::Config,
    pub base: super::Config,
}

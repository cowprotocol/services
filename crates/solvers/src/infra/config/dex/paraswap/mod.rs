pub mod file;

pub struct Config {
    pub paraswap: crate::infra::dex::paraswap::Config,
    pub base: super::Config,
}

pub mod file;

pub struct Config {
    pub okx: crate::infra::dex::okx::Config,
    pub base: super::Config,
}

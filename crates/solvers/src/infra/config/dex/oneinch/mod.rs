pub mod file;

pub struct Config {
    pub oneinch: crate::infra::dex::oneinch::Config,
    pub base: super::Config,
}

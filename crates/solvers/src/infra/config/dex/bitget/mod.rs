pub mod file;

pub struct Config {
    pub bitget: crate::infra::dex::bitget::Config,
    pub base: super::Config,
}

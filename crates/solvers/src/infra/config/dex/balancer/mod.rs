pub mod file;

pub struct Config {
    pub sor: crate::infra::dex::balancer::Config,
    pub base: super::Config,
}

use crate::domain;

pub mod file;

pub struct Config {
    pub risk: domain::Risk,
}

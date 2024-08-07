use crate::{
    arguments,
    boundary,
    domain::{self, fee::FeeFactor},
};

pub enum Policy {
    Surplus(Surplus),
    PriceImprovement(PriceImprovement),
    Volume(Volume),
}

pub struct Surplus {
    factor: FeeFactor,
    max_volume_factor: FeeFactor,
}

pub struct PriceImprovement {
    factor: FeeFactor,
    max_volume_factor: FeeFactor,
}

pub struct Volume {
    factor: FeeFactor,
}

impl From<arguments::FeePolicyKind> for Policy {
    fn from(policy_arg: arguments::FeePolicyKind) -> Self {
        match policy_arg {
            arguments::FeePolicyKind::Surplus {
                factor,
                max_volume_factor,
            } => Policy::Surplus(Surplus {
                factor,
                max_volume_factor,
            }),
            arguments::FeePolicyKind::PriceImprovement {
                factor,
                max_volume_factor,
            } => Policy::PriceImprovement(PriceImprovement {
                factor,
                max_volume_factor,
            }),
            arguments::FeePolicyKind::Volume { factor } => Policy::Volume(Volume { factor }),
        }
    }
}

impl Surplus {
    pub fn apply(&self, order: &boundary::Order) -> Option<domain::fee::Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => None,
            boundary::OrderClass::Liquidity => None,
            boundary::OrderClass::Limit => {
                let policy = domain::fee::Policy::Surplus {
                    factor: self.factor,
                    max_volume_factor: self.max_volume_factor,
                };
                Some(policy)
            }
        }
    }
}

impl PriceImprovement {
    pub fn apply(&self, order: &boundary::Order) -> Option<domain::fee::Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => None,
            boundary::OrderClass::Liquidity => None,
            boundary::OrderClass::Limit => Some(domain::fee::Policy::PriceImprovement {
                factor: self.factor,
                max_volume_factor: self.max_volume_factor,
            }),
        }
    }
}

impl Volume {
    pub fn apply(&self, order: &boundary::Order) -> Option<domain::fee::Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => None,
            boundary::OrderClass::Liquidity => None,
            boundary::OrderClass::Limit => Some(domain::fee::Policy::Volume {
                factor: self.factor,
            }),
        }
    }
}

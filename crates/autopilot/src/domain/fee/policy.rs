use crate::{arguments, boundary, domain};

pub enum Policy {
    Surplus(Surplus),
    PriceImprovement(PriceImprovement),
    Volume(Volume),
}

pub struct Surplus {
    factor: f64,
    max_volume_factor: f64,
}

pub struct PriceImprovement {
    factor: f64,
    max_volume_factor: f64,
}

pub struct Volume {
    factor: f64,
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
    pub fn apply(
        &self,
        order: &boundary::Order,
        quote: &domain::Quote,
    ) -> Option<domain::fee::Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => None,
            boundary::OrderClass::Liquidity => None,
            boundary::OrderClass::Limit => Some(domain::fee::Policy::PriceImprovement {
                factor: self.factor,
                max_volume_factor: self.max_volume_factor,
                quote: quote.clone().into(),
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

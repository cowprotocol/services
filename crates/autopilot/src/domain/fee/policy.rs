use {
    crate::{
        boundary,
        config::fee_policy::FeePolicyKind,
        domain::{self, fee::Quote},
    },
    shared::{fee::VolumeFeePolicy, fee_factor::FeeFactor},
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

impl From<FeePolicyKind> for Policy {
    fn from(policy_arg: FeePolicyKind) -> Self {
        match policy_arg {
            FeePolicyKind::Surplus {
                factor,
                max_volume_factor,
            } => Policy::Surplus(Surplus {
                factor,
                max_volume_factor,
            }),
            FeePolicyKind::PriceImprovement {
                factor,
                max_volume_factor,
            } => Policy::PriceImprovement(PriceImprovement {
                factor,
                max_volume_factor,
            }),
            FeePolicyKind::Volume { factor } => Policy::Volume(Volume { factor }),
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
                quote: Quote::from_domain(quote),
            }),
        }
    }
}

impl Volume {
    pub fn apply(
        &self,
        order: &boundary::Order,
        volume_fee_policy: &VolumeFeePolicy,
    ) -> Option<domain::fee::Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => None,
            boundary::OrderClass::Liquidity => None,
            boundary::OrderClass::Limit => {
                // Use shared function to determine applicable volume fee factor
                let factor = volume_fee_policy.get_applicable_volume_fee_factor(
                    order.data.buy_token,
                    order.data.sell_token,
                    Some(self.factor),
                )?;

                Some(domain::fee::Policy::Volume { factor })
            }
        }
    }
}

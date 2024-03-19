use {
    crate::{arguments, boundary, domain},
    app_data::Validator,
    prometheus::core::Number,
};

pub enum Policy {
    Surplus(Surplus),
    PriceImprovement(PriceImprovement),
    Volume(Volume),
}

pub struct Surplus {
    factor: f64,
    max_volume_factor: f64,
    skip_market_orders: bool,
}

pub struct PriceImprovement {
    factor: f64,
    max_volume_factor: f64,
}

pub struct Volume {
    factor: f64,
}

impl From<arguments::FeePolicy> for Policy {
    fn from(policy_arg: arguments::FeePolicy) -> Self {
        match policy_arg.fee_policy_kind {
            arguments::FeePolicyKind::Surplus {
                factor,
                max_volume_factor,
            } => Policy::Surplus(Surplus {
                factor,
                max_volume_factor,
                skip_market_orders: policy_arg.fee_policy_skip_market_orders,
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
    pub fn apply(
        &self,
        order: &boundary::Order,
        quote: &domain::Quote,
    ) -> Option<domain::fee::Policy> {
        match order.metadata.class {
            boundary::OrderClass::Market => None,
            boundary::OrderClass::Liquidity => None,
            boundary::OrderClass::Limit => {
                let policy = domain::fee::Policy::Surplus {
                    factor: self.factor,
                    max_volume_factor: self.max_volume_factor,
                };
                if !self.skip_market_orders {
                    Some(policy)
                } else {
                    let order_ = boundary::Amounts {
                        sell: order.data.sell_amount,
                        buy: order.data.buy_amount,
                        fee: order.data.fee_amount,
                    };
                    let quote_ = boundary::Amounts {
                        sell: quote.sell_amount,
                        buy: quote.buy_amount,
                        fee: quote.fee,
                    };

                    boundary::is_order_outside_market_price(&order_, &quote_).then_some(policy)
                }
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
        // If the partner fee is specified, it overwrites the current volume fee policy
        if let Some(validated_app_data) = order
            .metadata
            .full_app_data
            .as_ref()
            .map(|full_app_data| Validator::new(usize::MAX).validate(full_app_data.as_bytes()))
            .transpose()
            .ok()
            .flatten()
        {
            if let Some(partner_fee) = validated_app_data.protocol.partner_fee {
                return Some(domain::fee::Policy::Volume {
                    factor: partner_fee.bps.into_f64() / 100.0,
                });
            }
        }

        match order.metadata.class {
            boundary::OrderClass::Market => None,
            boundary::OrderClass::Liquidity => None,
            boundary::OrderClass::Limit => Some(domain::fee::Policy::Volume {
                factor: self.factor,
            }),
        }
    }
}

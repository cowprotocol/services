use {
    crate::{
        domain,
        domain::{eth, fee::FeeFactor},
    },
    shared::remaining_amounts,
};

pub fn to_domain(order: model::order::Order, quote: Option<domain::Quote>) -> domain::Order {
    let remaining_order = remaining_amounts::Order::from(order.clone());
    let order_is_untouched = remaining_order.executed_amount.is_zero();

    domain::Order {
        uid: order.metadata.uid.into(),
        sell: eth::Asset {
            token: order.data.sell_token.into(),
            amount: order.data.sell_amount.into(),
        },
        buy: eth::Asset {
            token: order.data.buy_token.into(),
            amount: order.data.buy_amount.into(),
        },
        protocol_fees: order
            .metadata
            .fee_policies
            .into_iter()
            .map(Into::into)
            .collect(),
        created: u32::try_from(order.metadata.creation_date.timestamp()).unwrap_or(u32::MIN),
        valid_to: order.data.valid_to,
        side: order.data.kind.into(),
        receiver: order.data.receiver.map(Into::into),
        owner: order.metadata.owner.into(),
        partially_fillable: order.data.partially_fillable,
        executed: remaining_order.executed_amount.into(),
        pre_interactions: order_is_untouched
            .then(|| order.interactions.pre.into_iter().map(Into::into).collect())
            .unwrap_or_default(),
        post_interactions: order
            .interactions
            .post
            .into_iter()
            .map(Into::into)
            .collect(),
        sell_token_balance: order.data.sell_token_balance.into(),
        buy_token_balance: order.data.buy_token_balance.into(),
        app_data: order.data.app_data.into(),
        signature: order.signature.into(),
        quote,
    }
}

impl From<model::fee_policy::FeePolicy> for domain::fee::Policy {
    fn from(policy: model::fee_policy::FeePolicy) -> Self {
        match policy {
            model::fee_policy::FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => domain::fee::Policy::Surplus {
                factor: FeeFactor::try_from(factor).unwrap(),
                max_volume_factor: FeeFactor::try_from(max_volume_factor).unwrap(),
            },
            model::fee_policy::FeePolicy::Volume { factor } => domain::fee::Policy::Volume {
                factor: FeeFactor::try_from(factor).unwrap(),
            },
            model::fee_policy::FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => {
                domain::fee::Policy::PriceImprovement {
                    factor: FeeFactor::try_from(factor).unwrap(),
                    max_volume_factor: FeeFactor::try_from(max_volume_factor).unwrap(),
                    quote: domain::fee::Quote {
                        sell_amount: quote.sell_amount,
                        buy_amount: quote.buy_amount,
                        fee: quote.fee,
                        solver: todo!(), // add solver to crate::model::fee_policy::Quote
                    },
                }
            }
        }
    }
}

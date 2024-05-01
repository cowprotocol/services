use {
    crate::{boundary, domain},
    database::fee_policies::{FeePolicy, FeePolicyKind},
};

pub fn from_domain(
    auction_id: domain::auction::Id,
    order_uid: domain::OrderUid,
    policy: domain::fee::Policy,
) -> FeePolicy {
    match policy {
        domain::fee::Policy::Surplus {
            factor,
            max_volume_factor,
        } => FeePolicy {
            auction_id,
            order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
            kind: FeePolicyKind::Surplus,
            surplus_factor: Some(factor.into()),
            surplus_max_volume_factor: Some(max_volume_factor.into()),
            volume_factor: None,
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        },
        domain::fee::Policy::Volume { factor } => FeePolicy {
            auction_id,
            order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
            kind: FeePolicyKind::Volume,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            volume_factor: Some(factor.into()),
            price_improvement_factor: None,
            price_improvement_max_volume_factor: None,
        },
        domain::fee::Policy::PriceImprovement {
            factor,
            max_volume_factor,
            quote: _,
        } => FeePolicy {
            auction_id,
            order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
            kind: FeePolicyKind::PriceImprovement,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            volume_factor: None,
            price_improvement_factor: Some(factor.into()),
            price_improvement_max_volume_factor: Some(max_volume_factor.into()),
        },
    }
}

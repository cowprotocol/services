use crate::{boundary, domain};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct FeePolicy {
    pub auction_id: database::auction::AuctionId,
    pub order_uid: boundary::database::OrderUid,
    pub kind: FeePolicyKind,
    pub surplus_factor: Option<f64>,
    pub surplus_max_volume_factor: Option<f64>,
    pub volume_factor: Option<f64>,
    pub price_improvement_factor: Option<f64>,
    pub price_improvement_max_volume_factor: Option<f64>,
}

impl FeePolicy {
    pub fn from_domain(
        auction_id: domain::auction::Id,
        order_uid: domain::OrderUid,
        policy: domain::fee::Policy,
    ) -> Self {
        match policy {
            domain::fee::Policy::Surplus {
                factor,
                max_volume_factor,
            } => Self {
                auction_id,
                order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
                kind: FeePolicyKind::Surplus,
                surplus_factor: Some(factor.into()),
                surplus_max_volume_factor: Some(max_volume_factor.into()),
                volume_factor: None,
                price_improvement_factor: None,
                price_improvement_max_volume_factor: None,
            },
            domain::fee::Policy::Volume { factor } => Self {
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
            } => Self {
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
}

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "PolicyKind", rename_all = "lowercase")]
pub enum FeePolicyKind {
    Surplus,
    Volume,
    PriceImprovement,
}

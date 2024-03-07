use crate::{boundary, domain};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct FeePolicy {
    pub auction_id: domain::AuctionId,
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
        auction_id: domain::AuctionId,
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
                surplus_factor: Some(factor),
                surplus_max_volume_factor: Some(max_volume_factor),
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
                volume_factor: Some(factor),
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
                price_improvement_factor: Some(factor),
                price_improvement_max_volume_factor: Some(max_volume_factor),
            },
        }
    }

    #[allow(dead_code)]
    pub fn into_domain(self, quote: Option<domain::fee::Quote>) -> domain::fee::Policy {
        match self.kind {
            FeePolicyKind::Surplus => domain::fee::Policy::Surplus {
                factor: self.surplus_factor.expect("missing surplus factor"),
                max_volume_factor: self
                    .surplus_max_volume_factor
                    .expect("missing max volume factor"),
            },
            FeePolicyKind::Volume => domain::fee::Policy::Volume {
                factor: self.volume_factor.expect("missing volume factor"),
            },
            FeePolicyKind::PriceImprovement => domain::fee::Policy::PriceImprovement {
                factor: self
                    .price_improvement_factor
                    .expect("missing price improvement factor"),
                max_volume_factor: self
                    .surplus_max_volume_factor
                    .expect("missing price improvement max volume factor"),
                quote: quote.expect("quote is required for the PriceImprovement policy fee"),
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

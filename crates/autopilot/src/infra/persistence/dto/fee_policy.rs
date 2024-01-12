use {crate::domain, database::byte_array::ByteArray};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct FeePolicy {
    pub auction_id: domain::AuctionId,
    pub order_uid: database::OrderUid,
    pub kind: FeePolicyKind,
    pub price_improvement_factor: Option<f64>,
    pub max_volume_factor: Option<f64>,
    pub volume_factor: Option<f64>,
}

impl FeePolicy {
    pub fn from_domain(
        auction_id: domain::AuctionId,
        order_uid: domain::OrderUid,
        policy: domain::fee::Policy,
    ) -> Self {
        match policy {
            domain::fee::Policy::PriceImprovement {
                factor,
                max_volume_factor,
            } => Self {
                auction_id,
                order_uid: ByteArray(order_uid.0),
                kind: FeePolicyKind::PriceImprovement,
                price_improvement_factor: Some(factor),
                max_volume_factor: Some(max_volume_factor),
                volume_factor: None,
            },
            domain::fee::Policy::Volume { factor } => Self {
                auction_id,
                order_uid: ByteArray(order_uid.0),
                kind: FeePolicyKind::Volume,
                price_improvement_factor: None,
                max_volume_factor: None,
                volume_factor: Some(factor),
            },
        }
    }
}

impl From<FeePolicy> for domain::fee::Policy {
    fn from(row: FeePolicy) -> domain::fee::Policy {
        match row.kind {
            FeePolicyKind::PriceImprovement => domain::fee::Policy::PriceImprovement {
                factor: row
                    .price_improvement_factor
                    .expect("missing price improvement factor"),
                max_volume_factor: row.max_volume_factor.expect("missing max volume factor"),
            },
            FeePolicyKind::Volume => domain::fee::Policy::Volume {
                factor: row.volume_factor.expect("missing volume factor"),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "PolicyKind", rename_all = "lowercase")]
pub enum FeePolicyKind {
    PriceImprovement,
    Volume,
}

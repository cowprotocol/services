use {
    crate::domain::{fee::Policy, AuctionId, OrderUid},
    database::{
        byte_array::ByteArray,
        fee_policies::{FeePolicyKindRow, FeePolicyRow},
    },
};

impl From<FeePolicyRow> for Policy {
    fn from(row: FeePolicyRow) -> Policy {
        match row.kind {
            FeePolicyKindRow::PriceImprovement => Policy::PriceImprovement {
                factor: row
                    .price_improvement_factor
                    .expect("missing price improvement factor"),
                max_volume_factor: row.max_volume_factor.expect("missing max volume factor"),
            },
            FeePolicyKindRow::Volume => Policy::Volume {
                factor: row.volume_factor.expect("missing volume factor"),
            },
        }
    }
}

pub fn from_domain(auction_id: AuctionId, order_uid: OrderUid, policy: Policy) -> FeePolicyRow {
    match policy {
        Policy::PriceImprovement {
            factor,
            max_volume_factor,
        } => FeePolicyRow {
            auction_id,
            order_uid: ByteArray(order_uid.0),
            kind: FeePolicyKindRow::PriceImprovement,
            price_improvement_factor: Some(factor),
            max_volume_factor: Some(max_volume_factor),
            volume_factor: None,
        },
        Policy::Volume { factor } => FeePolicyRow {
            auction_id,
            order_uid: ByteArray(order_uid.0),
            kind: FeePolicyKindRow::Volume,
            price_improvement_factor: None,
            max_volume_factor: None,
            volume_factor: Some(factor),
        },
    }
}

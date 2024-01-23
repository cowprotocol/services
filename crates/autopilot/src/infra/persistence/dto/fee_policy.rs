use {
    crate::{boundary, domain},
    bigdecimal::BigDecimal,
    number::conversions::{big_decimal_to_u256, u256_to_big_decimal},
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct FeePolicy {
    pub auction_id: domain::AuctionId,
    pub order_uid: boundary::database::OrderUid,
    pub kind: FeePolicyKind,
    pub surplus_factor: Option<f64>,
    pub max_volume_factor: Option<f64>,
    pub volume_factor: Option<f64>,
    pub quote_sell_amount: Option<BigDecimal>,
    pub quote_buy_amount: Option<BigDecimal>,
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
                max_volume_factor: Some(max_volume_factor),
                volume_factor: None,
                quote_sell_amount: None,
                quote_buy_amount: None,
            },
            domain::fee::Policy::Volume { factor } => Self {
                auction_id,
                order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
                kind: FeePolicyKind::Volume,
                surplus_factor: None,
                max_volume_factor: None,
                volume_factor: Some(factor),
                quote_sell_amount: None,
                quote_buy_amount: None,
            },
            domain::fee::Policy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => Self {
                auction_id,
                order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
                kind: FeePolicyKind::Surplus,
                surplus_factor: Some(factor),
                max_volume_factor: Some(max_volume_factor),
                volume_factor: None,
                quote_sell_amount: Some(u256_to_big_decimal(&quote.sell_amount)),
                quote_buy_amount: Some(u256_to_big_decimal(&quote.buy_amount)),
            },
        }
    }
}

impl From<FeePolicy> for domain::fee::Policy {
    fn from(row: FeePolicy) -> domain::fee::Policy {
        match row.kind {
            FeePolicyKind::Surplus => domain::fee::Policy::Surplus {
                factor: row.surplus_factor.expect("missing surplus factor"),
                max_volume_factor: row.max_volume_factor.expect("missing max volume factor"),
            },
            FeePolicyKind::Volume => domain::fee::Policy::Volume {
                factor: row.volume_factor.expect("missing volume factor"),
            },
            FeePolicyKind::PriceImprovement => domain::fee::Policy::PriceImprovement {
                factor: row.surplus_factor.expect("missing surplus factor"),
                max_volume_factor: row.max_volume_factor.expect("missing max volume factor"),
                quote: domain::fee::Quote {
                    sell_amount: big_decimal_to_u256(
                        &row.quote_sell_amount.expect("missing sell amount"),
                    )
                    .expect("sell amount is not a valid eth::U256"),
                    buy_amount: big_decimal_to_u256(
                        &row.quote_buy_amount.expect("missing buy amount"),
                    )
                    .expect("buy amount is not a valid eth::U256"),
                },
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

use {
    crate::{boundary, domain},
    anyhow::anyhow,
    bigdecimal::BigDecimal,
    number::conversions::{big_decimal_to_u256, u256_to_big_decimal},
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct FeePolicy {
    pub auction_id: domain::AuctionId,
    pub order_uid: boundary::database::OrderUid,
    pub kind: FeePolicyKind,
    pub surplus_factor: Option<f64>,
    pub surplus_max_volume_factor: Option<f64>,
    pub volume_factor: Option<f64>,
    pub price_improvement_factor: Option<f64>,
    pub price_improvement_volume_factor: Option<f64>,
    pub price_improvement_quote_sell_amount: Option<BigDecimal>,
    pub price_improvement_quote_buy_amount: Option<BigDecimal>,
    pub price_improvement_quote_fee: Option<BigDecimal>,
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
                price_improvement_volume_factor: None,
                price_improvement_quote_sell_amount: None,
                price_improvement_quote_buy_amount: None,
                price_improvement_quote_fee: None,
            },
            domain::fee::Policy::Volume { factor } => Self {
                auction_id,
                order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
                kind: FeePolicyKind::Volume,
                surplus_factor: None,
                surplus_max_volume_factor: None,
                volume_factor: Some(factor),
                price_improvement_factor: None,
                price_improvement_volume_factor: None,
                price_improvement_quote_sell_amount: None,
                price_improvement_quote_buy_amount: None,
                price_improvement_quote_fee: None,
            },
            domain::fee::Policy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => Self {
                auction_id,
                order_uid: boundary::database::byte_array::ByteArray(order_uid.0),
                kind: FeePolicyKind::PriceImprovement,
                surplus_factor: None,
                surplus_max_volume_factor: None,
                volume_factor: None,
                price_improvement_factor: Some(factor),
                price_improvement_volume_factor: Some(max_volume_factor),
                price_improvement_quote_sell_amount: Some(u256_to_big_decimal(&quote.sell_amount)),
                price_improvement_quote_buy_amount: Some(u256_to_big_decimal(&quote.buy_amount)),
                price_improvement_quote_fee: Some(u256_to_big_decimal(&quote.fee)),
            },
        }
    }
}

impl From<FeePolicy> for domain::fee::Policy {
    fn from(row: FeePolicy) -> domain::fee::Policy {
        match row.kind {
            FeePolicyKind::Surplus => domain::fee::Policy::Surplus {
                factor: row.surplus_factor.expect("missing surplus factor"),
                max_volume_factor: row
                    .surplus_max_volume_factor
                    .expect("missing max volume factor"),
            },
            FeePolicyKind::Volume => domain::fee::Policy::Volume {
                factor: row.volume_factor.expect("missing volume factor"),
            },
            FeePolicyKind::PriceImprovement => domain::fee::Policy::PriceImprovement {
                factor: row
                    .price_improvement_factor
                    .expect("missing price improvement factor"),
                max_volume_factor: row
                    .surplus_max_volume_factor
                    .expect("missing price improvement max volume factor"),
                quote: domain::fee::Quote {
                    sell_amount: row
                        .price_improvement_quote_sell_amount
                        .ok_or(anyhow!("missing price improvement quote sell amount"))
                        .and_then(|sell_amount| {
                            big_decimal_to_u256(&sell_amount).ok_or(anyhow!(
                                "price improvement quote sell amount is not a valid BigDecimal"
                            ))
                        })
                        .unwrap(),
                    buy_amount: row
                        .price_improvement_quote_buy_amount
                        .ok_or(anyhow!("missing price improvement quote buy amount"))
                        .and_then(|sell_amount| {
                            big_decimal_to_u256(&sell_amount).ok_or(anyhow!(
                                "price improvement quote buy amount is not a valid BigDecimal"
                            ))
                        })
                        .unwrap(),
                    fee: row
                        .price_improvement_quote_fee
                        .ok_or(anyhow!("missing price improvement quote fee"))
                        .and_then(|sell_amount| {
                            big_decimal_to_u256(&sell_amount).ok_or(anyhow!(
                                "price improvement quote fee is not a valid BigDecimal"
                            ))
                        })
                        .unwrap(),
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

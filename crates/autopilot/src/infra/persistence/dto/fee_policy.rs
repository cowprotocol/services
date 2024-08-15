use {
    crate::{boundary, domain},
    anyhow::Context,
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

pub fn try_into_domain(
    policy: FeePolicy,
    quote: Option<&domain::quote::Quote>,
) -> Result<domain::fee::Policy, Error> {
    let policy = match policy.kind {
        FeePolicyKind::Surplus => domain::fee::Policy::Surplus {
            factor: policy
                .surplus_factor
                .context("missing surplus_factor")?
                .try_into()?,
            max_volume_factor: policy
                .surplus_max_volume_factor
                .context("missing surplus_max_volume_factor")?
                .try_into()?,
        },
        FeePolicyKind::Volume => domain::fee::Policy::Volume {
            factor: policy
                .volume_factor
                .context("missing volume_factor")?
                .try_into()?,
        },
        FeePolicyKind::PriceImprovement => domain::fee::Policy::PriceImprovement {
            factor: policy
                .price_improvement_factor
                .context("missing price_improvement_factor")?
                .try_into()?,
            max_volume_factor: policy
                .price_improvement_max_volume_factor
                .context("missing price_improvement_max_volume_factor")?
                .try_into()?,
            quote: {
                let quote = quote.ok_or(Error::MissingQuote)?;
                domain::fee::Quote {
                    sell_amount: quote.sell_amount.into(),
                    buy_amount: quote.buy_amount.into(),
                    fee: quote.fee.into(),
                    solver: quote.solver.into(),
                }
            },
        },
    };
    Ok(policy)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to convert database data to domain data {0}")]
    Inconsistency(#[from] anyhow::Error),
    #[error("missing quote for price improvement fee policy")]
    MissingQuote,
}

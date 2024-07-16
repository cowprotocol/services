use {
    anyhow::Context,
    bigdecimal::{BigDecimal, FromPrimitive},
    database::{auction::AuctionId, OrderUid},
    model::fee_policy::{FeePolicy, Quote},
    number::conversions::big_decimal_to_u256,
};

impl super::Postgres {
    pub async fn fee_policies(
        &self,
        auction_id: AuctionId,
        order_uid: OrderUid,
        quote: Option<&database::orders::Quote>,
    ) -> anyhow::Result<Vec<FeePolicy>> {
        let mut ex = self.pool.acquire().await?;

        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fee_policies"])
            .start_timer();

        let fee_policies = database::fee_policies::fetch(&mut ex, auction_id, order_uid).await?;
        fee_policies
            .into_iter()
            .map(|db_fee_policy| fee_policy_from(db_fee_policy, quote, order_uid))
            .collect::<Result<Vec<_>, _>>()
    }
}

fn fee_policy_from(
    db_fee_policy: database::fee_policies::FeePolicy,
    quote: Option<&database::orders::Quote>,
    order_uid: OrderUid,
) -> anyhow::Result<FeePolicy> {
    Ok(match db_fee_policy.kind {
        database::fee_policies::FeePolicyKind::Surplus => FeePolicy::Surplus {
            factor: db_fee_policy
                .surplus_factor
                .context("missing surplus factor")?,
            max_volume_factor: db_fee_policy
                .surplus_max_volume_factor
                .context("missing surplus max volume factor")?,
        },
        database::fee_policies::FeePolicyKind::Volume => FeePolicy::Volume {
            factor: db_fee_policy
                .volume_factor
                .context("missing volume factor")?,
        },
        database::fee_policies::FeePolicyKind::PriceImprovement => {
            let quote = quote.context(format!(
                "missing price improvement quote for order '{:?}'",
                order_uid
            ))?;
            let fee = quote.gas_amount * quote.gas_price / quote.sell_token_price;
            FeePolicy::PriceImprovement {
                factor: db_fee_policy
                    .price_improvement_factor
                    .context("missing price improvement factor")?,
                max_volume_factor: db_fee_policy
                    .price_improvement_max_volume_factor
                    .context("missing price improvement max volume factor")?,
                quote: Quote {
                    sell_amount: big_decimal_to_u256(&quote.sell_amount)
                        .context("invalid price improvement quote sell amount value")?,
                    buy_amount: big_decimal_to_u256(&quote.buy_amount)
                        .context("invalid price improvement quote buy amount value")?,
                    fee: BigDecimal::from_f64(fee)
                        .as_ref()
                        .and_then(big_decimal_to_u256)
                        .context("invalid price improvement quote fee value")?,
                },
            }
        }
    })
}

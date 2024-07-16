use {
    anyhow::Context,
    bigdecimal::{BigDecimal, FromPrimitive},
    database::{auction::AuctionId, OrderUid},
    model::fee_policy::{FeePolicy, Quote},
    number::conversions::big_decimal_to_u256,
    std::collections::HashMap,
};

impl super::Postgres {
    pub async fn fee_policies(
        &self,
        keys_filter: &[(AuctionId, OrderUid)],
    ) -> anyhow::Result<HashMap<(AuctionId, OrderUid), Vec<FeePolicy>>> {
        let mut ex = self.pool.acquire().await?;

        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fee_policies"])
            .start_timer();

        let fee_policies = database::fee_policies::fetch(&mut ex, keys_filter).await?;
        let quote_order_uids = fee_policies
            .iter()
            .filter_map(|((_, order_uid), policies)| {
                policies
                    .iter()
                    .any(|policy| {
                        matches!(
                            policy.kind,
                            database::fee_policies::FeePolicyKind::PriceImprovement
                        )
                    })
                    .then_some(*order_uid)
            })
            .collect::<Vec<_>>();

        let timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["order_quotes"])
            .start_timer();
        let quotes = database::orders::read_quotes(&mut ex, quote_order_uids.as_slice())
            .await?
            .into_iter()
            .map(|quote| (quote.order_uid, quote))
            .collect::<HashMap<_, _>>();
        timer.stop_and_record();

        fee_policies
            .into_iter()
            .map(|((auction_id, order_uid), policies)| {
                policies
                    .into_iter()
                    .map(|policy| fee_policy_from(policy, quotes.get(&order_uid), order_uid))
                    .collect::<anyhow::Result<Vec<_>>>()
                    .map(|policies| ((auction_id, order_uid), policies))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()
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

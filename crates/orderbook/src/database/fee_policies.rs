use {
    anyhow::Context,
    bigdecimal::{
        num_traits::{CheckedDiv, CheckedMul},
        FromPrimitive,
    },
    database::{auction::AuctionId, OrderUid},
    model::fee_policy::{FeePolicy, Quote},
    num::BigRational,
    number::conversions::{big_decimal_to_u256, big_rational_to_u256},
    std::collections::HashMap,
};

impl super::Postgres {
    pub async fn fee_policies(
        &self,
        keys_filter: &[(AuctionId, OrderUid)],
    ) -> anyhow::Result<HashMap<(AuctionId, OrderUid), Vec<FeePolicy>>> {
        let mut ex = self.pool.acquire().await?;

        let timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fee_policies"])
            .start_timer();
        let fee_policies = database::fee_policies::fetch_all(&mut ex, keys_filter).await?;
        timer.stop_and_record();

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
            let gas_amount =
                BigRational::from_f64(quote.gas_amount).context("invalid quote gas amount")?;
            let gas_price =
                BigRational::from_f64(quote.gas_price).context("invalid quote gas price")?;
            let sell_token_price = BigRational::from_f64(quote.sell_token_price)
                .context("invalid quote sell token price")?;
            let fee = big_rational_to_u256(
                &gas_amount
                    .checked_mul(&gas_price)
                    .context("gas amount and gas price multiplication overflow")?
                    .checked_div(&sell_token_price)
                    .context("invalid price improvement quote fee value")?,
            )?;
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
                    fee,
                },
            }
        }
    })
}

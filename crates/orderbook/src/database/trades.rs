use {
    crate::database::Postgres,
    anyhow::{Context, Result},
    bigdecimal::{BigDecimal, FromPrimitive},
    database::{byte_array::ByteArray, trades::TradesQueryRow},
    ethcontract::H160,
    futures::{stream::TryStreamExt, StreamExt},
    model::{
        fee_policy::{FeePolicy, Quote},
        order::OrderUid,
        trade::Trade,
    },
    number::conversions::{big_decimal_to_big_uint, big_decimal_to_u256},
    primitive_types::H256,
    std::convert::TryInto,
};

#[async_trait::async_trait]
pub trait TradeRetrieving: Send + Sync {
    async fn trades(&self, filter: &TradeFilter) -> Result<Vec<Trade>>;
}

/// Any default value means that this field is unfiltered.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct TradeFilter {
    pub owner: Option<H160>,
    pub order_uid: Option<OrderUid>,
}

#[async_trait::async_trait]
impl TradeRetrieving for Postgres {
    async fn trades(&self, filter: &TradeFilter) -> Result<Vec<Trade>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["trades"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        database::trades::trades(
            &mut ex,
            filter.owner.map(|owner| ByteArray(owner.0)).as_ref(),
            filter.order_uid.map(|uid| ByteArray(uid.0)).as_ref(),
        )
        .map(|result| match result {
            Ok(row) => trade_from(row),
            Err(err) => Err(anyhow::Error::from(err)),
        })
        .try_collect()
        .await
    }
}

fn trade_from(row: TradesQueryRow) -> Result<Trade> {
    let block_number = row
        .block_number
        .try_into()
        .context("block_number is not u32")?;
    let log_index = row.log_index.try_into().context("log_index is not u32")?;
    let order_uid = OrderUid(row.order_uid.0);
    let buy_amount = big_decimal_to_big_uint(&row.buy_amount)
        .context("buy_amount is not an unsigned integer")?;
    let sell_amount = big_decimal_to_big_uint(&row.sell_amount)
        .context("sell_amount is not an unsigned integer")?;
    let sell_amount_before_fees = big_decimal_to_big_uint(&row.sell_amount_before_fees)
        .context("sell_amount_before_fees is not an unsigned integer")?;
    let owner = H160(row.owner.0);
    let buy_token = H160(row.buy_token.0);
    let sell_token = H160(row.sell_token.0);
    let tx_hash = row.tx_hash.map(|hash| H256(hash.0));
    let fee_policies = row
        .fee_policies
        .into_iter()
        .map(|policy| fee_policy_from(policy, row.quote.as_ref()))
        .collect::<Result<Vec<FeePolicy>>>()?;
    Ok(Trade {
        block_number,
        log_index,
        order_uid,
        buy_amount,
        sell_amount,
        sell_amount_before_fees,
        owner,
        buy_token,
        sell_token,
        tx_hash,
        fee_policies,
    })
}

fn fee_policy_from(
    db_fee_policy: database::fee_policies::FeePolicy,
    quote: Option<&database::trades::Quote>,
) -> Result<FeePolicy> {
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
            let quote = quote.context("missing price improvement quote")?;
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
                    fee: BigDecimal::from_f64(quote.fee)
                        .as_ref()
                        .and_then(big_decimal_to_u256)
                        .context("invalid price improvement quote fee value")?,
                },
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_trade() {
        trade_from(TradesQueryRow::default()).unwrap();
    }
}

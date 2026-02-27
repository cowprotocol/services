//! Database access via PostgreSQL.

use {
    crate::traits::DbRead,
    alloy::primitives::Address,
    anyhow::{Context, Result},
    contracts::CoWSwapEthFlow::EthFlowOrder,
    database::{
        OrderUid,
        ethflow_orders::{EthOrderPlacement, read_order, refundable_orders},
        orders::read_order as read_db_order,
    },
    number::conversions::big_decimal_to_u256,
    sqlx::PgPool,
    std::time::Duration,
};

/// [`DbRead`] implementation using PostgreSQL.
pub struct Postgres {
    pool: PgPool,
    lookback_time: Option<Duration>,
}

impl Postgres {
    pub fn new(pool: PgPool, lookback_time: Option<Duration>) -> Self {
        Self {
            pool,
            lookback_time,
        }
    }
}

impl DbRead for Postgres {
    async fn get_refundable_orders(
        &self,
        block_time: i64,
        min_validity_duration: i64,
        min_price_deviation: f64,
    ) -> Result<Vec<EthOrderPlacement>> {
        let mut ex = self.pool.acquire().await?;
        refundable_orders(
            &mut ex,
            block_time,
            min_validity_duration,
            min_price_deviation,
            self.lookback_time,
        )
        .await
        .context("Error while retrieving the refundable ethflow orders from db")
    }

    async fn get_ethflow_order_data(&self, uid: &OrderUid) -> Result<EthFlowOrder::Data> {
        let mut ex = self
            .pool
            .acquire()
            .await
            .with_context(|| format!("acquire connection for uid {uid:?}"))?;
        let order = read_db_order(&mut ex, uid)
            .await
            .with_context(|| format!("read order {uid:?}"))?
            .with_context(|| format!("missing order {uid:?}"))?;
        let ethflow_order = read_order(&mut ex, uid)
            .await
            .with_context(|| format!("read ethflow order {uid:?}"))?
            .with_context(|| format!("missing ethflow order {uid:?}"))?;

        let receiver = order
            .receiver
            .with_context(|| format!("order {uid:?} missing receiver"))?;
        let sell_amount = big_decimal_to_u256(&order.sell_amount)
            .with_context(|| format!("order {uid:?} invalid sell_amount"))?;
        let buy_amount = big_decimal_to_u256(&order.buy_amount)
            .with_context(|| format!("order {uid:?} invalid buy_amount"))?;
        let fee_amount = big_decimal_to_u256(&order.fee_amount)
            .with_context(|| format!("order {uid:?} invalid fee_amount"))?;

        Ok(EthFlowOrder::Data {
            buyToken: Address::from(order.buy_token.0),
            receiver: Address::from(receiver.0),
            sellAmount: sell_amount,
            buyAmount: buy_amount,
            appData: order.app_data.0.into(),
            feeAmount: fee_amount,
            validTo: ethflow_order.valid_to as u32,
            partiallyFillable: order.partially_fillable,
            quoteId: 0i64, // quoteId is not important for refunding and will be ignored
        })
    }
}

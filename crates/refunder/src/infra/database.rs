//! PostgreSQL implementation of [`DbRead`].

use {
    crate::traits::DbRead,
    alloy::primitives::Address,
    anyhow::{Context, Result},
    contracts::alloy::CoWSwapEthFlow::EthFlowOrder,
    database::{
        OrderUid,
        ethflow_orders::{EthOrderPlacement, read_order, refundable_orders},
        orders::read_order as read_db_order,
    },
    number::conversions::alloy::big_decimal_to_u256,
    sqlx::PgPool,
};

/// PostgreSQL implementation of [`DbRead`].
pub struct Postgres {
    pool: PgPool,
}

impl Postgres {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
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
        )
        .await
        .context("Error while retrieving the refundable ethflow orders from db")
    }

    async fn get_ethflow_order_data(&self, uid: &OrderUid) -> Result<EthFlowOrder::Data> {
        let mut ex = self.pool.acquire().await.context("acquire")?;
        let order = read_db_order(&mut ex, uid)
            .await
            .context("read order")?
            .context("missing order")?;
        let ethflow_order = read_order(&mut ex, uid)
            .await
            .context("read ethflow order")?
            .context("missing ethflow order")?;

        Ok(EthFlowOrder::Data {
            buyToken: Address::from(order.buy_token.0),
            receiver: Address::from(order.receiver.unwrap().0),
            sellAmount: big_decimal_to_u256(&order.sell_amount).unwrap(),
            buyAmount: big_decimal_to_u256(&order.buy_amount).unwrap(),
            appData: order.app_data.0.into(),
            feeAmount: big_decimal_to_u256(&order.fee_amount).unwrap(),
            validTo: ethflow_order.valid_to as u32,
            partiallyFillable: order.partially_fillable,
            quoteId: 0i64, // quoteId is not important for refunding and will be ignored
        })
    }
}

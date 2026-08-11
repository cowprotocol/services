use {
    super::Listener,
    crate::{
        domain::OrderUid,
        infra::{
            self,
            persistence::{Persistence, dto::order::from_domain},
            solvers::dto::settle,
        },
    },
    alloy::primitives::{Address, U256},
    anyhow::Context,
    ethrpc::block_stream::CurrentBlockWatcher,
    price_estimation::native::{NativePriceEstimating, to_normalized_price},
    std::{collections::HashMap, sync::Arc, time::Duration},
};

/// Settles fast-path (out-of-competition) orders. When a fast-path order
/// arrives, it re-encodes the order's cached quote solution through the
/// driver's `/settle`, filling at the winning solution's executed amounts.
pub struct FastPathSettler {
    persistence: Persistence,
    drivers: Vec<Arc<infra::Driver>>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    current_block: CurrentBlockWatcher,
    /// Blocks added to the current block to bound the settlement submission.
    submission_deadline: u64,
    settle_timeout: Duration,
    native_price_timeout: Duration,
}

impl FastPathSettler {
    pub fn new(
        persistence: Persistence,
        drivers: Vec<Arc<infra::Driver>>,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
        current_block: CurrentBlockWatcher,
        submission_deadline: u64,
        settle_timeout: Duration,
        native_price_timeout: Duration,
    ) -> Self {
        Self {
            persistence,
            drivers,
            native_price_estimator,
            current_block,
            submission_deadline,
            settle_timeout,
            native_price_timeout,
        }
    }

    async fn try_settle(&self, uid: OrderUid) -> anyhow::Result<()> {
        let Some(fast_path) = self.persistence.fast_path_order(uid).await? else {
            return Ok(());
        };
        let driver = self
            .drivers
            .iter()
            .find(|driver| driver.submission_address == fast_path.solver)
            .with_context(|| format!("no driver for fast-path solver {:?}", fast_path.solver))?;

        let native_prices = self
            .native_prices(&[
                fast_path.order.sell.token.into(),
                fast_path.order.buy.token.into(),
            ])
            .await;

        let request = settle::Request {
            solution_id: fast_path.solution_id,
            submission_deadline_latest_block: self.current_block.borrow().number
                + self.submission_deadline,
            auction_id: fast_path.auction_id,
            fast_path: Some(settle::FastPath {
                order: from_domain(&fast_path.order),
                limit_prices: settle::LimitPrices {
                    sell: fast_path.limit_sell,
                    buy: fast_path.limit_buy,
                },
                native_prices,
            }),
        };
        driver.settle(&request, self.settle_timeout).await
    }

    /// Best-effort native prices for the order's tokens; tokens without a price
    /// are omitted (the driver tolerates a partial map).
    async fn native_prices(&self, tokens: &[Address]) -> HashMap<Address, U256> {
        let mut prices = HashMap::new();
        for &token in tokens {
            match self
                .native_price_estimator
                .estimate_native_price(token, self.native_price_timeout)
                .await
            {
                Ok(price) => {
                    if let Some(price) = to_normalized_price(price) {
                        prices.insert(token, price);
                    }
                }
                Err(err) => tracing::warn!(?token, ?err, "no native price for fast-path token"),
            }
        }
        prices
    }
}

#[async_trait::async_trait]
impl Listener for FastPathSettler {
    async fn on_new_order(&self, order: OrderUid) {
        if let Err(err) = self.try_settle(order).await {
            tracing::warn!(?order, ?err, "fast-path settle failed");
        }
    }
}

use {
    super::Listener,
    crate::{
        domain::OrderUid,
        infra::{
            self,
            persistence::{FastPathOrder, Persistence, dto::order::from_domain},
            solvers::dto::settle,
        },
    },
    alloy::primitives::{Address, U256},
    anyhow::Context,
    ethrpc::block_stream::CurrentBlockWatcher,
    price_estimation::native::{NativePriceEstimating, to_normalized_price},
    std::{collections::HashMap, sync::Arc, time::Duration},
};

/// Settles fast-path (out-of-competition) orders by re-encoding the order's
/// cached quote solution through the driver's `/settle`.
#[derive(Clone)]
pub struct FastPathSettler {
    persistence: Persistence,
    drivers: Vec<Arc<infra::Driver>>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    current_block: CurrentBlockWatcher,
    /// Blocks added to the current block to bound the settlement submission.
    submission_deadline: u64,
    settle_timeout: Duration,
}

impl FastPathSettler {
    pub fn new(
        persistence: Persistence,
        drivers: Vec<Arc<infra::Driver>>,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
        current_block: CurrentBlockWatcher,
        submission_deadline: u64,
        settle_timeout: Duration,
    ) -> Self {
        Self {
            persistence,
            drivers,
            native_price_estimator,
            current_block,
            submission_deadline,
            settle_timeout,
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

        let deadline = self.current_block.borrow().number + self.submission_deadline;
        tracing::info!(
            ?uid,
            solver = ?fast_path.solver,
            auction_id = fast_path.auction_id,
            solution_id = fast_path.solution_id,
            "initiating fast-path settle"
        );
        let request = build_settle_request(&fast_path, native_prices, deadline);
        driver.settle(&request, self.settle_timeout).await
    }

    /// Native prices for the order's tokens, from the estimator's cache only.
    /// Missing prices are omitted; the driver tolerates a partial map.
    async fn native_prices(&self, tokens: &[Address]) -> HashMap<Address, U256> {
        let mut prices = HashMap::new();
        for &token in tokens {
            match self
                .native_price_estimator
                .estimate_native_price(token, Duration::ZERO)
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

/// Build the driver `/settle` request for a recovered fast-path order.
fn build_settle_request(
    fast_path: &FastPathOrder,
    native_prices: HashMap<Address, U256>,
    submission_deadline_latest_block: u64,
) -> settle::Request {
    settle::Request {
        solution_id: fast_path.solution_id,
        submission_deadline_latest_block,
        auction_id: fast_path.auction_id,
        fast_path: Some(settle::FastPath {
            order: from_domain(&fast_path.order),
            limit_prices: settle::LimitPrices {
                sell: fast_path.limit_sell,
                buy: fast_path.limit_buy,
            },
            native_prices,
        }),
    }
}

#[async_trait::async_trait]
impl Listener for FastPathSettler {
    async fn on_new_order(&self, order: OrderUid) {
        // Run detached so a slow settle doesn't hold up the notification loop.
        let this = self.clone();
        tokio::spawn(async move {
            if let Err(err) = this.try_settle(order).await {
                tracing::warn!(?order, ?err, "fast-path settle failed");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{database::Postgres, infra::persistence::Persistence},
        bigdecimal::BigDecimal,
        configs::autopilot::solver::Account,
        database::{
            byte_array::ByteArray,
            orders::{Order as DbOrder, OrderKind, Quote},
        },
        eth_domain_types as eth,
    };

    /// Seeds a fast-path competition, then checks the that autopilot recovers
    /// it, routes to the driver whose submission address is the winning
    /// solver, and builds a `/settle` filling at the recorded amounts.
    #[tokio::test]
    #[ignore]
    async fn postgres_fast_path_settle_recovers_routes_and_builds() {
        let postgres = Arc::new(Postgres::with_defaults().await.unwrap());
        let persistence = Persistence::new(None, postgres.clone()).await;

        let mut tx = postgres.pool.begin().await.unwrap();
        database::clear_DANGER_(&mut tx).await.unwrap();

        let auction_id = 42;
        let solution_id = 999;
        let order_uid = ByteArray([1u8; 56]);
        let regular_uid = ByteArray([2u8; 56]);
        let solver = ByteArray([3u8; 20]);

        for uid in [order_uid, regular_uid] {
            database::orders::insert_order(
                &mut tx,
                &DbOrder {
                    uid,
                    kind: OrderKind::Sell,
                    signature: vec![0u8; 65],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        }
        database::orders::insert_quote(
            &mut tx,
            &Quote {
                order_uid,
                auction_id: Some(auction_id),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        database::orders::insert_quote(
            &mut tx,
            &Quote {
                order_uid: regular_uid,
                auction_id: None,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Winning solution; the driver's solution id lives in `id`, not `uid`.
        database::solver_competition_v2::save(
            &mut tx,
            auction_id,
            &[database::solver_competition_v2::Solution {
                uid: 0,
                id: BigDecimal::from(solution_id),
                solver,
                is_winner: true,
                orders: vec![database::solver_competition_v2::Order {
                    uid: order_uid,
                    executed_sell: BigDecimal::from(111),
                    executed_buy: BigDecimal::from(222),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Recover.
        let fast_path = persistence
            .fast_path_order(OrderUid(order_uid.0))
            .await
            .unwrap()
            .expect("fast-path order recovered");
        assert_eq!(fast_path.auction_id, auction_id);
        assert_eq!(fast_path.solution_id, solution_id as u64); // from `id`, not `uid`
        assert_eq!(fast_path.solver, eth::Address::from([3u8; 20]));
        assert_eq!(fast_path.limit_sell, U256::from(111u64));
        assert_eq!(fast_path.limit_buy, U256::from(222u64));

        // Route to the driver whose submission address is the winning solver.
        let matching = Arc::new(
            infra::Driver::try_new(
                "http://matching".parse().unwrap(),
                "matching".into(),
                Account::Address(fast_path.solver),
            )
            .await
            .unwrap(),
        );
        let other = Arc::new(
            infra::Driver::try_new(
                "http://other".parse().unwrap(),
                "other".into(),
                Account::Address(eth::Address::from([9u8; 20])),
            )
            .await
            .unwrap(),
        );
        let drivers = [other, matching];
        let selected = drivers
            .iter()
            .find(|driver| driver.submission_address == fast_path.solver)
            .expect("a driver matches the solver");
        assert_eq!(selected.name, "matching");

        // Build: fill exactly at the recorded amounts.
        let native_prices = HashMap::from([(Address::from([3u8; 20]), U256::from(1u64))]);
        let request = build_settle_request(&fast_path, native_prices.clone(), 12_345);
        assert_eq!(request.solution_id, solution_id as u64);
        assert_eq!(request.auction_id, auction_id);
        assert_eq!(request.submission_deadline_latest_block, 12_345);
        let fast_path = request.fast_path.expect("fastPath present");
        assert_eq!(fast_path.limit_prices.sell, U256::from(111u64));
        assert_eq!(fast_path.limit_prices.buy, U256::from(222u64));
        assert_eq!(fast_path.native_prices, native_prices);

        // A regular quoted order (no auction_id) is not fast-path.
        assert!(
            persistence
                .fast_path_order(OrderUid(regular_uid.0))
                .await
                .unwrap()
                .is_none()
        );
    }
}

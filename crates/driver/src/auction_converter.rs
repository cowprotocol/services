use {
    anyhow::{Context, Result},
    gas_estimation::GasPriceEstimating,
    model::{auction::AuctionWithId as AuctionModel, TokenPair},
    primitive_types::H160,
    shared::recent_block_cache::Block,
    solver::{
        liquidity::order_converter::OrderConverter,
        liquidity_collector::LiquidityCollecting,
        settlement::external_prices::ExternalPrices,
        solver::Auction,
    },
    std::{
        collections::HashSet,
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    },
};

// TODO eventually this has to be part of the auction coming from the autopilot.
/// Determines how much time a solver has to compute solutions for an incoming
/// `Auction`.
const RUN_DURATION: Duration = Duration::from_secs(15);

#[async_trait::async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait AuctionConverting: Send + Sync {
    async fn convert_auction(&self, model: AuctionModel, block: u64) -> Result<Auction>;
}

pub struct AuctionConverter {
    pub order_converter: Arc<OrderConverter>,
    pub gas_price_estimator: Arc<dyn GasPriceEstimating>,
    pub native_token: H160,
    pub run: AtomicU64,
    pub liquidity_collector: Box<dyn LiquidityCollecting>,
}

impl AuctionConverter {
    pub fn new(
        gas_price_estimator: Arc<dyn GasPriceEstimating>,
        liquidity_collector: Box<dyn LiquidityCollecting>,
        order_converter: Arc<OrderConverter>,
    ) -> Self {
        Self {
            gas_price_estimator,
            native_token: order_converter.native_token.address(),
            run: AtomicU64::default(),
            liquidity_collector,
            order_converter,
        }
    }
}

#[async_trait::async_trait]
impl AuctionConverting for AuctionConverter {
    async fn convert_auction(&self, auction: AuctionModel, block: u64) -> Result<Auction> {
        let auction_id = auction.id;
        let auction = auction.auction;
        let run = self.run.fetch_add(1, Ordering::SeqCst);
        let orders = auction
            .orders
            .into_iter()
            .filter_map(|order| {
                let uid = order.metadata.uid;
                match self.order_converter.normalize_limit_order(order) {
                    Ok(mut order)
                        if order.buy_amount != 0.into() && order.sell_amount != 0.into() =>
                    {
                        order.reward = auction.rewards.get(&uid).copied().unwrap_or(0.);
                        Some(order)
                    }
                    Err(err) => {
                        // This should never happen unless we are getting malformed
                        // orders from the API - so raise an alert if this happens.
                        tracing::error!(?err, "error normalizing limit order");
                        None
                    }
                    _ => {
                        // TODO: Find out why those orders are not an issue in the old driver.
                        // Those orders cause errors inside quasimodo.
                        let err = anyhow::anyhow!("but_amount or sell_amount is 0");
                        tracing::error!(?err, "error normalizing limit order");
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        anyhow::ensure!(
            orders.iter().any(|o| !o.is_liquidity_order()),
            "auction contains no user orders"
        );

        tracing::info!(?orders, "got {} orders", orders.len());

        let token_pairs: HashSet<_> = orders
            .iter()
            .filter(|o| !o.is_liquidity_order())
            .flat_map(|o| TokenPair::new(o.buy_token, o.sell_token))
            .collect();

        let liquidity = self
            .liquidity_collector
            .get_liquidity(token_pairs, Block::Number(block))
            .await?;

        let external_prices =
            ExternalPrices::try_from_auction_prices(self.native_token, auction.prices)
                .context("malformed acution prices")?;
        tracing::debug!(?external_prices, "estimated prices");

        let gas_price = self
            .gas_price_estimator
            .estimate()
            .await
            .context("failed to estimate gas price")?;
        tracing::debug!("solving with gas price of {:?}", gas_price);

        Ok(Auction {
            id: auction_id,
            run,
            orders,
            liquidity,
            liquidity_fetch_block: block,
            gas_price: gas_price.effective_gas_price(),
            deadline: Instant::now() + RUN_DURATION,
            external_prices,
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        contracts::WETH9,
        gas_estimation::GasPrice1559,
        maplit::btreemap,
        model::{
            order::{Order, OrderClass, OrderData, OrderMetadata, BUY_ETH_ADDRESS},
            TokenPair,
        },
        num::rational::{BigRational, Ratio},
        primitive_types::U256,
        shared::{dummy_contract, gas_price_estimation::FakeGasPriceEstimator},
        solver::{
            liquidity::{
                AmmOrderExecution,
                ConstantProductOrder,
                Liquidity::ConstantProduct,
                SettlementHandling,
            },
            liquidity_collector::MockLiquidityCollecting,
            settlement::SettlementEncoder,
        },
    };

    struct DummySettlementHandler;
    impl SettlementHandling<ConstantProductOrder> for DummySettlementHandler {
        fn encode(
            &self,
            _execution: AmmOrderExecution,
            _encoder: &mut SettlementEncoder,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn converts_auction() {
        let token = H160::from_low_u64_be;
        let order = |sell_token, buy_token, with_error| Order {
            data: OrderData {
                sell_token: token(sell_token),
                buy_token: token(buy_token),
                buy_amount: 10.into(),
                sell_amount: 10.into(),
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                full_fee_amount: 100.into(),
                executed_buy_amount: if with_error { 100u8 } else { 1u8 }.into(),
                class: OrderClass::Market,
                ..Default::default()
            },
            ..Default::default()
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 0.0,
            max_fee_per_gas: 10000.0,
            max_priority_fee_per_gas: 10000.0,
        };
        let gas_estimator = Arc::new(FakeGasPriceEstimator::new(gas_price));
        let native_token = dummy_contract!(WETH9, token(1));
        let mut liquidity_collector = MockLiquidityCollecting::new();
        liquidity_collector
            .expect_get_liquidity()
            .times(2)
            .withf(move |pairs, block| {
                let expected: HashSet<_> = [
                    TokenPair::new(token(1), token(2)).unwrap(),
                    TokenPair::new(token(2), token(3)).unwrap(),
                ]
                .into_iter()
                .collect();
                expected == *pairs && block == &Block::Number(3)
            })
            .returning(move |_, _| {
                Ok(vec![ConstantProduct(ConstantProductOrder {
                    address: H160::from_low_u64_be(1),
                    tokens: TokenPair::new(token(1), token(2)).unwrap(),
                    reserves: (1u128, 1u128),
                    fee: Ratio::<u32>::new(1, 1),
                    settlement_handling: Arc::new(DummySettlementHandler),
                })])
            });
        let order_converter = Arc::new(OrderConverter {
            native_token: native_token.clone(),
            fee_objective_scaling_factor: 2.,
            min_order_age: Duration::from_secs(30),
        });
        let converter = AuctionConverter::new(
            gas_estimator,
            Box::new(liquidity_collector),
            order_converter,
        );
        let mut model = AuctionModel {
            id: 3,
            auction: model::auction::Auction {
                block: 1,
                latest_settlement_block: 2,
                orders: vec![order(1, 2, false), order(2, 3, false), order(1, 3, true)],
                prices: btreemap! { token(2) => U256::exp10(18), token(3) => U256::exp10(18) },
                rewards: Default::default(),
            },
        };

        let auction = converter.convert_auction(model.clone(), 3).await.unwrap();
        assert_eq!(auction.id, 3);
        assert_eq!(
            auction
                .deadline
                .duration_since(Instant::now())
                .as_secs_f64()
                .ceil(),
            RUN_DURATION.as_secs_f64()
        );
        assert_eq!(auction.run, 0);
        // only orders which don't have a logical error
        assert_eq!(auction.orders.len(), 2);
        assert_eq!(auction.orders[0].sell_token, token(1));
        assert_eq!(auction.orders[0].buy_token, token(2));
        assert_eq!(auction.orders[1].sell_token, token(2));
        assert_eq!(auction.orders[1].buy_token, token(3));

        // 100 total fee of 10% filled order with fee factor of 2.0 == 180 scaled fee
        assert_eq!(auction.orders[0].scaled_unsubsidized_fee, 180.into());
        assert_eq!(auction.orders[1].scaled_unsubsidized_fee, 180.into());
        assert_eq!(auction.liquidity.len(), 1);
        for t in &[native_token.address(), BUY_ETH_ADDRESS, token(2), token(3)] {
            assert_eq!(
                auction.external_prices.price(t).unwrap(),
                &BigRational::from_float(1.).unwrap()
            );
        }

        let auction = converter.convert_auction(model.clone(), 3).await.unwrap();
        assert_eq!(auction.run, 1);

        // auction has to include at least 1 user order
        model.auction.orders = vec![order(1, 2, false)];
        model.auction.orders[0].metadata.class = OrderClass::Liquidity;
        assert!(converter.convert_auction(model, 3).await.is_err());
    }
}

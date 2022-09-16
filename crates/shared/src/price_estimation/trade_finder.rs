//! Module with shared logic for creating a `PriceEstimating` implementation
//! from an inner `TradeFinding`.

use super::{
    rate_limited, Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
};
use crate::{
    code_simulation::CodeSimulating,
    rate_limiter::RateLimiter,
    request_sharing::RequestSharing,
    trade_finding::{Trade, TradeError, TradeFinding},
    transport::extensions::StateOverride,
    web3_traits::CodeFetching,
};
use anyhow::{ensure, Context as _, Result};
use contracts::support::{AnyoneAuthenticator, PhonyERC20, Trader};
use ethcontract::{tokens::Tokenize, H160, I256, U256};
use futures::{
    future::{BoxFuture, FutureExt as _},
    stream::StreamExt as _,
};
use maplit::hashmap;
use model::order::OrderKind;
use std::sync::Arc;
use web3::{ethabi::Token, types::CallRequest};

/// A `TradeFinding`-based price estimator with request sharing and rate
/// limiting.
pub struct TradeEstimator {
    inner: Inner,
    sharing: RequestSharing<Query, BoxFuture<'static, Result<Estimate, PriceEstimationError>>>,
    rate_limiter: Arc<RateLimiter>,
}

#[derive(Clone)]
struct Inner {
    finder: Arc<dyn TradeFinding>,
    verifier: Option<TradeVerifier>,
}

/// A trade verifier.
#[derive(Clone)]
pub struct TradeVerifier {
    simulator: Arc<dyn CodeSimulating>,
    code_fetcher: Arc<dyn CodeFetching>,
    authenticator: H160,
}

impl TradeEstimator {
    pub fn new(finder: Arc<dyn TradeFinding>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            inner: Inner {
                finder,
                verifier: None,
            },
            sharing: Default::default(),
            rate_limiter,
        }
    }

    pub fn with_verifier(mut self, verifier: TradeVerifier) -> Self {
        self.inner.verifier = Some(verifier);
        self
    }

    async fn estimate(&self, query: Query) -> Result<Estimate, PriceEstimationError> {
        let estimate = rate_limited(
            self.rate_limiter.clone(),
            self.inner.clone().estimate(query),
        );
        self.sharing.shared(query, estimate.boxed()).await
    }
}

impl Inner {
    async fn estimate(self, query: Query) -> Result<Estimate, PriceEstimationError> {
        let trade = self.finder.get_trade(&query).await?;
        match self.verifier {
            Some(verifier) => verifier
                .verify(query, trade)
                .await
                .map_err(PriceEstimationError::Other),
            None => Ok(Estimate {
                out_amount: trade.out_amount,
                gas: trade.gas_estimate,
            }),
        }
    }
}

impl TradeVerifier {
    const DEFAULT_TRADER: H160 = addr!("Ca1Fca1fcA1FCA1fcA1fcA1fca1Fca1FcA1fCA1f");
    const DEFAULT_ORIGIN: H160 = addr!("BEeFbeefbEefbeEFbeEfbEEfBEeFbeEfBeEfBeef");
    const DEFAULT_GAS: u64 = 8_000_000;

    const TOKEN_IMPLEMENTATION: H160 = addr!("0000000000000000000000000000000000010000");

    pub fn new(
        simulator: Arc<dyn CodeSimulating>,
        code_fetcher: Arc<dyn CodeFetching>,
        authenticator: H160,
    ) -> Self {
        Self {
            simulator,
            code_fetcher,
            authenticator,
        }
    }

    async fn verify(&self, query: Query, trade: Trade) -> Result<Estimate> {
        let trader = dummy_contract!(Trader, Self::DEFAULT_TRADER);
        let sell_token_code = self.code_fetcher.code(query.sell_token).await?;

        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Sell => (query.in_amount, slippage::sub(trade.out_amount)),
            OrderKind::Buy => (slippage::add(trade.out_amount), query.in_amount),
        };

        let tx = trader
            .methods()
            .settle(
                vec![query.sell_token, query.buy_token],
                vec![buy_amount, sell_amount],
                trade.encode(),
                sell_amount,
            )
            .tx;

        let call = CallRequest {
            from: Some(Self::DEFAULT_ORIGIN),
            to: tx.to,
            data: tx.data,
            gas: Some(Self::DEFAULT_GAS.into()),
            ..Default::default()
        };
        let overrides = hashmap! {
            trader.address() => StateOverride {
                code: Some(deployed_bytecode!(Trader)),
                ..Default::default()
            },
            self.authenticator => StateOverride {
                code: Some(deployed_bytecode!(AnyoneAuthenticator)),
                ..Default::default()
            },
            query.sell_token => StateOverride {
                code: Some(deployed_bytecode!(PhonyERC20)),
                ..Default::default()
            },
            Self::TOKEN_IMPLEMENTATION => StateOverride {
                code: Some(sell_token_code),
                ..Default::default()
            },
        };

        let return_data = self.simulator.simulate(call, overrides).await?;
        let output = SettleOutput::decode(&return_data)?;

        let trader_amounts = output
            .trader_amounts()
            .context("trade simulation output missing trader token balances")?;
        ensure!(
            trader_amounts == (sell_amount, buy_amount),
            "mismatched amounts transferred to traders"
        );

        let (executed_sell_amount, executed_buy_amount) = output
            .executed_amounts()
            .context("trade simulation output missing settlement token balances")?;
        let out_amount = match query.kind {
            OrderKind::Sell => {
                ensure!(
                    executed_sell_amount <= query.in_amount,
                    "trade simulation sold more than input"
                );
                executed_buy_amount
            }
            OrderKind::Buy => {
                ensure!(
                    executed_buy_amount >= query.in_amount,
                    "trade simulation bought less than input"
                );
                executed_sell_amount
            }
        };

        Ok(Estimate {
            gas: output.gas_used.as_u64(),
            out_amount,
        })
    }
}

impl PriceEstimating for TradeEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        futures::stream::iter(queries)
            .then(|query| self.estimate(*query))
            .enumerate()
            .boxed()
    }
}

impl From<TradeError> for PriceEstimationError {
    fn from(err: TradeError) -> Self {
        match err {
            TradeError::NoLiquidity => Self::NoLiquidity,
            TradeError::UnsupportedOrderType => Self::UnsupportedOrderType,
            TradeError::Other(err) => Self::Other(err),
        }
    }
}

/// Output of `Trader::settle` smart contract call.
#[derive(Debug, Eq, PartialEq)]
struct SettleOutput {
    gas_used: U256,
    trader_balances: Vec<I256>,
    settlement_balances: Vec<I256>,
}

impl SettleOutput {
    fn decode(output: &[u8]) -> Result<Self> {
        let function = Trader::raw_contract().abi.function("settle").unwrap();
        let tokens = function.decode_output(output)?;
        let (gas_used, trader_balances, settlement_balances) =
            Tokenize::from_token(Token::Tuple(tokens))?;
        Ok(Self {
            gas_used,
            trader_balances,
            settlement_balances,
        })
    }

    fn trader_amounts(&self) -> Option<(U256, U256)> {
        Some((
            self.trader_balances.first()?.wrapping_abs().into_raw(),
            self.trader_balances.last()?.into_raw(),
        ))
    }

    fn executed_amounts(&self) -> Option<(U256, U256)> {
        Some((
            self.trader_balances
                .first()?
                .checked_add(*self.settlement_balances.first()?)?
                .wrapping_abs()
                .into_raw(),
            self.trader_balances
                .last()?
                .checked_add(*self.settlement_balances.last()?)?
                .into_raw(),
        ))
    }
}

/// Module for adding **generous** slippage to trade simulations. The slippage
/// is very generous since we return the executed trade amounts and use those
/// for the final estimate.
mod slippage {
    use crate::conversions::U256Ext as _;
    use ethcontract::U256;

    fn abs(amount: U256) -> U256 {
        amount.ceil_div(&100.into())
    }

    pub fn add(amount: U256) -> U256 {
        amount.saturating_add(abs(amount))
    }

    pub fn sub(amount: U256) -> U256 {
        amount.saturating_sub(abs(amount)).max(U256::one())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        code_simulation::TenderlyCodeSimlator, price_estimation::single_estimate,
        tenderly_api::TenderlyHttpApi, trade_finding::zeroex::ZeroExTradeFinder,
        transport::create_env_test_transport, zeroex_api::DefaultZeroExApi, Web3,
    };
    use hex_literal::hex;

    #[test]
    fn decodes_trader_settle_output() {
        let output = SettleOutput::decode(&hex!(
            "0000000000000000000000000000000000000000000000000000000000000539
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000e0
             0000000000000000000000000000000000000000000000000000000000000003
             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000003
             0000000000000000000000000000000000000000000000000000000000000003
             0000000000000000000000000000000000000000000000000000000000000000
             fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc"
        ))
        .unwrap();

        assert_eq!(
            output,
            SettleOutput {
                gas_used: 1337.into(),
                trader_balances: vec![(-1).into(), 0.into(), 2.into()],
                settlement_balances: vec![3.into(), 0.into(), (-4).into()],
            }
        );
    }

    #[test]
    fn computes_executed_amounts() {
        let output = SettleOutput {
            gas_used: 1337.into(),
            trader_balances: vec![(-1_000_000).into(), 999_000.into()],
            settlement_balances: vec![1_111.into(), (-2).into()],
        };

        let (executed_sell_amount, executed_buy_amount) = output.executed_amounts().unwrap();

        // Trader transferred out 1_000_000, and 1_111 stayed behind in the
        // settlement contract, so executed amount is:
        assert_eq!(executed_sell_amount, U256::from(998_889));

        // Trader received 999_000 and contract paid 2 of that amount from its
        // buffers, so executed amount is:
        assert_eq!(executed_buy_amount, U256::from(998_998));
    }

    #[tokio::test]
    #[ignore]
    async fn verified_zeroex_trade() {
        let web3 = Web3::new(create_env_test_transport());

        let tenderly_api = TenderlyHttpApi::test_from_env();
        let simulator = TenderlyCodeSimlator::new(tenderly_api, 1).save(true, true);
        let verifier = TradeVerifier::new(
            Arc::new(simulator),
            Arc::new(web3),
            addr!("2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE"),
        );

        let zeroex_api = DefaultZeroExApi::test();
        let finder = ZeroExTradeFinder::new(Arc::new(zeroex_api), vec![]);

        let estimator =
            TradeEstimator::new(Arc::new(finder), RateLimiter::test()).with_verifier(verifier);

        let estimate = single_estimate(
            &estimator,
            &Query {
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::GNO,
                in_amount: 10u128.pow(18).into(),
                kind: OrderKind::Sell,
            },
        )
        .await
        .unwrap();

        println!(
            "1.0 WETH buys {} GNO, costing {} gas",
            estimate.out_amount.to_f64_lossy() / 1e18,
            estimate.gas,
        );
    }
}

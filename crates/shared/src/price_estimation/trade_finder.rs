//! Module with shared logic for creating a `PriceEstimating` implementation
//! from an inner `TradeFinding`.

use super::{
    rate_limited, Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
};
use crate::{
    code_fetching::CodeFetching,
    code_simulation::{CodeSimulating, SimulationError},
    rate_limiter::RateLimiter,
    request_sharing::RequestSharing,
    trade_finding::{Trade, TradeError, TradeFinding},
    transport::extensions::StateOverride,
};
use anyhow::{bail, ensure, Context as _, Result};
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
        match self.verifier {
            Some(verifier) => {
                let trade = self.finder.get_trade(&query).await?;
                verifier
                    .verify(query, trade)
                    .await
                    .map_err(PriceEstimationError::Other)
            }
            None => {
                let quote = self.finder.get_quote(&query).await?;
                Ok(Estimate {
                    out_amount: quote.out_amount,
                    gas: quote.gas_estimate,
                })
            }
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
        let trader = dummy_contract!(Trader, query.from.unwrap_or(Self::DEFAULT_TRADER));
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
            // Setup up our trader code that actually executes the settlement
            trader.address() => StateOverride {
                code: Some(deployed_bytecode!(Trader)),
                ..Default::default()
            },
            // Override the CoW protocol solver authenticator with one that
            // allows any address to solve
            self.authenticator => StateOverride {
                code: Some(deployed_bytecode!(AnyoneAuthenticator)),
                ..Default::default()
            },
            // Override the sell token with a phony facade for minting tokens to
            // the trader, so they have enough balance to execute the swap.
            query.sell_token => StateOverride {
                code: Some(deployed_bytecode!(PhonyERC20)),
                ..Default::default()
            },
            // Include the original token implementation that the phony token
            // facade can proxy to.
            Self::TOKEN_IMPLEMENTATION => StateOverride {
                code: Some(sell_token_code),
                ..Default::default()
            },
        };

        let return_data = match self.simulator.simulate(call, overrides).await {
            Ok(data) => data,
            Err(SimulationError::Other(err)) => {
                // In case we have a simulator error (network, service is down,
                // etc.), we optimistically return the quote estimate without
                // simulation. This is so we don't accidentally stop allowing
                // all quotes because the API we use for simulations is down.
                tracing::warn!(?err, "trade simulation error");
                return Ok(Estimate {
                    out_amount: trade.out_amount,
                    gas: trade.gas_estimate,
                });
            }
            Err(err) => bail!(err),
        };
        let output = SettleOutput::decode(&return_data)?;

        let trader_amounts = output
            .trader_amounts()
            .context("trade simulation output missing trader token balances")?;
        ensure!(
            trader_amounts == (sell_amount, buy_amount),
            "mismatched amounts transferred to trader"
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
        code_fetching::MockCodeFetching,
        code_simulation::{MockCodeSimulating, TenderlyCodeSimulator},
        price_estimation::single_estimate,
        tenderly_api::TenderlyHttpApi,
        trade_finding::{zeroex::ZeroExTradeFinder, Interaction, MockTradeFinding, Quote},
        transport::create_env_test_transport,
        zeroex_api::DefaultZeroExApi,
        Web3,
    };
    use anyhow::anyhow;
    use hex_literal::hex;
    use mockall::predicate;
    use std::sync::Mutex;

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
    async fn simulates_trades() {
        let authenticator = H160([0xa; 20]);
        let query = Query {
            from: Some(H160([0x1; 20])),
            sell_token: H160([0x2; 20]),
            buy_token: H160([0x3; 20]),
            in_amount: 1_000_000_u128.into(),
            kind: OrderKind::Sell,
        };
        let trade = Trade {
            out_amount: 2_000_000_u128.into(),
            gas_estimate: 133_700,
            approval: None,
            interaction: Interaction {
                target: H160([0x7; 20]),
                value: 0_u64.into(),
                data: vec![1, 2, 3, 4],
            },
        };
        let sell_token_code = bytes!("05060708");

        // settle(
        //     [0x0202..0202, 0x0303..0303],
        //     [1_980_000, 1_000_000],
        //     [
        //         [],
        //         [(0x0707..0707, 0, 0x01020304)],
        //         [],
        //     ],
        //     1_000_000,
        // )
        let call = CallRequest {
            from: Some(TradeVerifier::DEFAULT_ORIGIN),
            to: query.from,
            gas: Some(TradeVerifier::DEFAULT_GAS.into()),
            data: Some(bytes!(
                "299de750
                 0000000000000000000000000000000000000000000000000000000000000080
                 00000000000000000000000000000000000000000000000000000000000000e0
                 0000000000000000000000000000000000000000000000000000000000000140
                 00000000000000000000000000000000000000000000000000000000000f4240
                 0000000000000000000000000000000000000000000000000000000000000002
                 0000000000000000000000000202020202020202020202020202020202020202
                 0000000000000000000000000303030303030303030303030303030303030303
                 0000000000000000000000000000000000000000000000000000000000000002
                 00000000000000000000000000000000000000000000000000000000001e3660
                 00000000000000000000000000000000000000000000000000000000000f4240
                 0000000000000000000000000000000000000000000000000000000000000060
                 0000000000000000000000000000000000000000000000000000000000000080
                 0000000000000000000000000000000000000000000000000000000000000160
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000001
                 0000000000000000000000000000000000000000000000000000000000000020
                 0000000000000000000000000707070707070707070707070707070707070707
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000060
                 0000000000000000000000000000000000000000000000000000000000000004
                 0102030400000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000000"
            )),
            ..Default::default()
        };
        let overrides = hashmap! {
            query.from.unwrap() => StateOverride {
                code: Some(deployed_bytecode!(Trader)),
                ..Default::default()
            },
            authenticator => StateOverride {
                code: Some(deployed_bytecode!(AnyoneAuthenticator)),
                ..Default::default()
            },
            query.sell_token => StateOverride {
                code: Some(deployed_bytecode!(PhonyERC20)),
                ..Default::default()
            },
            TradeVerifier::TOKEN_IMPLEMENTATION => StateOverride {
                code: Some(sell_token_code.clone()),
                ..Default::default()
            },
        };

        // (
        //     420_000,
        //     [-1_000_000, 1_980_000],
        //     [0, 19_000],
        // )
        let output = bytes!(
            "00000000000000000000000000000000000000000000000000000000000668a0
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000c0
             0000000000000000000000000000000000000000000000000000000000000002
             fffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0bdc0
             00000000000000000000000000000000000000000000000000000000001e3660
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000004a38"
        );

        let mut finder = MockTradeFinding::new();
        finder
            .expect_get_trade()
            .with(predicate::eq(query))
            .returning({
                let trade = trade.clone();
                move |_| Ok(trade.clone())
            });

        let mut simulator = MockCodeSimulating::new();
        simulator
            .expect_simulate()
            .with(predicate::eq(call), predicate::eq(overrides))
            .returning(move |_, _| Ok(output.clone().0));

        let mut code_fetcher = MockCodeFetching::new();
        code_fetcher
            .expect_code()
            .with(predicate::eq(query.sell_token))
            .returning({
                let code = sell_token_code.clone();
                move |_| Ok(code.clone())
            });

        let estimator = TradeEstimator::new(Arc::new(finder), RateLimiter::test()).with_verifier(
            TradeVerifier::new(Arc::new(simulator), Arc::new(code_fetcher), authenticator),
        );

        let estimate = single_estimate(&estimator, &query).await.unwrap();

        assert_eq!(
            estimate,
            Estimate {
                out_amount: 1_999_000_u128.into(),
                gas: 420_000,
            }
        );
    }

    #[tokio::test]
    async fn adds_slippage_for_buy_trade() {
        let query = Query {
            from: None,
            sell_token: H160([0x1; 20]),
            buy_token: H160([0x2; 20]),
            in_amount: 2_000_000_u128.into(),
            kind: OrderKind::Buy,
        };
        let trade = Trade {
            out_amount: 1_000_000_u128.into(),
            ..Default::default()
        };

        // settle(
        //     [0x0101..0101, 0x0202..0202],
        //     [2_000_000, 1_010_000],
        //     [
        //         [],
        //         [(0x0, 0, 0x)],
        //         [],
        //     ],
        //     1_000_000,
        // )
        let call = CallRequest {
            from: Some(TradeVerifier::DEFAULT_ORIGIN),
            to: Some(TradeVerifier::DEFAULT_TRADER),
            gas: Some(TradeVerifier::DEFAULT_GAS.into()),
            data: Some(bytes!(
                "299de750
                 0000000000000000000000000000000000000000000000000000000000000080
                 00000000000000000000000000000000000000000000000000000000000000e0
                 0000000000000000000000000000000000000000000000000000000000000140
                 00000000000000000000000000000000000000000000000000000000000f6950
                 0000000000000000000000000000000000000000000000000000000000000002
                 0000000000000000000000000101010101010101010101010101010101010101
                 0000000000000000000000000202020202020202020202020202020202020202
                 0000000000000000000000000000000000000000000000000000000000000002
                 00000000000000000000000000000000000000000000000000000000001e8480
                 00000000000000000000000000000000000000000000000000000000000f6950
                 0000000000000000000000000000000000000000000000000000000000000060
                 0000000000000000000000000000000000000000000000000000000000000080
                 0000000000000000000000000000000000000000000000000000000000000140
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000001
                 0000000000000000000000000000000000000000000000000000000000000020
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000060
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000000"
            )),
            ..Default::default()
        };

        // (
        //     0,
        //     [-1_010_000, 2_000_000],
        //     [0, 0],
        // )
        let output = bytes!(
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000c0
             0000000000000000000000000000000000000000000000000000000000000002
             fffffffffffffffffffffffffffffffffffffffffffffffffffffffffff096b0
             00000000000000000000000000000000000000000000000000000000001e8480
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000"
        );

        let mut finder = MockTradeFinding::new();
        finder.expect_get_trade().returning({
            let trade = trade.clone();
            move |_| Ok(trade.clone())
        });

        let mut simulator = MockCodeSimulating::new();
        simulator
            .expect_simulate()
            .with(predicate::eq(call), predicate::always())
            .returning(move |_, _| Ok(output.clone().0));

        let mut code_fetcher = MockCodeFetching::new();
        code_fetcher
            .expect_code()
            .returning(|_| Ok(Default::default()));

        let estimator = TradeEstimator::new(Arc::new(finder), RateLimiter::test()).with_verifier(
            TradeVerifier::new(
                Arc::new(simulator),
                Arc::new(code_fetcher),
                Default::default(),
            ),
        );

        let estimate = single_estimate(&estimator, &query).await.unwrap();

        assert_eq!(
            estimate,
            Estimate {
                out_amount: 1_010_000_u128.into(),
                gas: 0,
            }
        );
    }

    #[tokio::test]
    async fn estimates_with_quote_without_verifier() {
        let query = Query {
            from: Some(H160([0x1; 20])),
            sell_token: H160([0x2; 20]),
            buy_token: H160([0x3; 20]),
            in_amount: 1_000_000_u128.into(),
            kind: OrderKind::Sell,
        };
        let quote = Quote {
            out_amount: 2_000_000_u128.into(),
            gas_estimate: 133_700,
        };

        let mut finder = MockTradeFinding::new();
        finder
            .expect_get_quote()
            .with(predicate::eq(query))
            .returning({
                let quote = quote.clone();
                move |_| Ok(quote.clone())
            });

        let estimator = TradeEstimator::new(Arc::new(finder), RateLimiter::test());

        let estimate = single_estimate(&estimator, &query).await.unwrap();

        assert_eq!(
            estimate,
            Estimate {
                out_amount: 2_000_000_u128.into(),
                gas: 133_700,
            }
        );
    }

    #[tokio::test]
    async fn ignores_non_revert_simulation_errors() {
        let query = Query {
            from: Some(H160([0x1; 20])),
            sell_token: H160([0x2; 20]),
            buy_token: H160([0x3; 20]),
            in_amount: 1_000_000_u128.into(),
            kind: OrderKind::Sell,
        };
        let trade = Trade {
            out_amount: 2_000_000_u128.into(),
            gas_estimate: 133_700,
            ..Default::default()
        };

        let mut finder = MockTradeFinding::new();
        finder.expect_get_trade().returning({
            let trade = trade.clone();
            move |_| Ok(trade.clone())
        });

        let mut simulator = MockCodeSimulating::new();
        simulator
            .expect_simulate()
            .returning(move |_, _| Err(SimulationError::Other(anyhow!("connection error"))));

        let mut code_fetcher = MockCodeFetching::new();
        code_fetcher
            .expect_code()
            .returning(|_| Ok(Default::default()));

        let estimator = TradeEstimator::new(Arc::new(finder), RateLimiter::test()).with_verifier(
            TradeVerifier::new(
                Arc::new(simulator),
                Arc::new(code_fetcher),
                Default::default(),
            ),
        );

        let estimate = single_estimate(&estimator, &query).await.unwrap();

        assert_eq!(
            estimate,
            Estimate {
                out_amount: 2_000_000_u128.into(),
                gas: 133_700,
            }
        );
    }

    #[tokio::test]
    async fn traded_and_executed_amount_checks() {
        let query = Query {
            from: None,
            sell_token: H160([0x1; 20]),
            buy_token: H160([0x2; 20]),
            in_amount: 100_u64.into(),
            kind: OrderKind::Sell,
        };
        let trade = Trade {
            out_amount: 203_u64.into(),
            ..Default::default()
        };

        let output = Arc::new(Mutex::new(bytes!("")));

        let mut finder = MockTradeFinding::new();
        finder
            .expect_get_trade()
            .returning(move |_| Ok(trade.clone()));

        let mut simulator = MockCodeSimulating::new();
        simulator.expect_simulate().returning({
            let output = output.clone();
            move |_, _| Ok(output.lock().unwrap().clone().0)
        });

        let mut code_fetcher = MockCodeFetching::new();
        code_fetcher.expect_code().returning(|_| Ok(bytes!("")));

        let estimator = TradeEstimator::new(Arc::new(finder), RateLimiter::test()).with_verifier(
            TradeVerifier::new(
                Arc::new(simulator),
                Arc::new(code_fetcher),
                Default::default(),
            ),
        );

        macro_rules! assert_output {
            ($check:ident: $x:literal) => {
                *output.lock().unwrap() = bytes!($x);
                assert!(single_estimate(&estimator, &query).await.$check());
            };
        }

        // Cannot decode output
        assert_output!(is_err: "");

        // Mising trader balances
        //
        // (
        //     0,
        //     [0],
        //     [0, 0],
        // )
        assert_output!(
            is_err:
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000a0
             0000000000000000000000000000000000000000000000000000000000000001
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000"
        );

        // Mising settlement balances
        //
        // (
        //     0,
        //     [0, 0],
        //     [0],
        // )
        assert_output!(
            is_err:
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000c0
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000001
             0000000000000000000000000000000000000000000000000000000000000000"
        );

        // Executed the exact trade amounts.
        //
        // (
        //     0,
        //     [-100, 200],
        //     [0, 3],
        // )
        assert_output!(
            is_ok:
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000c0
             0000000000000000000000000000000000000000000000000000000000000002
             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff9c
             00000000000000000000000000000000000000000000000000000000000000c8
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000003"
        );

        // Traded with positive slippage
        //
        // (
        //     0,
        //     [-100, 200],
        //     [0, 10],
        // )
        assert_output!(
            is_ok:
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000c0
             0000000000000000000000000000000000000000000000000000000000000002
             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff9c
             00000000000000000000000000000000000000000000000000000000000000c8
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             000000000000000000000000000000000000000000000000000000000000000a"
        );

        // Traded with negative slippage
        //
        // (
        //     0,
        //     [-100, 200],
        //     [0, -1],
        // )
        assert_output!(
            is_ok:
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000c0
             0000000000000000000000000000000000000000000000000000000000000002
             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff9c
             00000000000000000000000000000000000000000000000000000000000000c8
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        );

        // Trader balance changed by something other than the clearing amounts
        // (even if it would be beneficial).
        //
        // (
        //     0,
        //     [-99, 200],
        //     [0, 0],
        // )
        assert_output!(
            is_err:
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000c0
             0000000000000000000000000000000000000000000000000000000000000002
             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff9d
             00000000000000000000000000000000000000000000000000000000000000c8
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000"
        );

        // Executed amount does not match trade (here, a sell trade for 100
        // tokens only traded 99).
        //
        // (
        //     0,
        //     [-100, 200],
        //     [1, 0],
        // )
        assert_output!(
            is_err:
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000000c0
             0000000000000000000000000000000000000000000000000000000000000002
             ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff9d
             00000000000000000000000000000000000000000000000000000000000000c8
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn verified_zeroex_trade() {
        let web3 = Web3::new(create_env_test_transport());

        let tenderly_api = TenderlyHttpApi::test_from_env();
        let simulator = TenderlyCodeSimulator::new(tenderly_api, 1).save(true, true);
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
                from: Some(addr!("A03be496e67Ec29bC62F01a428683D7F9c204930")),
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::COW,
                in_amount: 10u128.pow(18).into(),
                kind: OrderKind::Sell,
            },
        )
        .await
        .unwrap();

        println!(
            "1.0 WETH buys {} COW, costing {} gas",
            estimate.out_amount.to_f64_lossy() / 1e18,
            estimate.gas,
        );
    }
}

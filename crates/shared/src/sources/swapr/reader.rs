//! A pool state reading implementation specific to Swapr.

use {
    crate::sources::uniswap_v2::pool_fetching::{self, DefaultPoolReader, Pool, PoolReading},
    anyhow::Result,
    contracts::ISwaprPair,
    ethcontract::{errors::MethodError, BlockId},
    futures::{future::BoxFuture, FutureExt as _},
    model::TokenPair,
    num::rational::Ratio,
};

/// A specialized Uniswap-like pool reader for DXdao Swapr pools.
///
/// Specifically, Swapr pools have dynamic fees that need to be fetched with the
/// pool state.
pub struct SwaprPoolReader(pub DefaultPoolReader);

/// The base amount for fees representing 100%.
const FEE_BASE: u32 = 10_000;

impl PoolReading for SwaprPoolReader {
    fn read_state(&self, pair: TokenPair, block: BlockId) -> BoxFuture<'_, Result<Option<Pool>>> {
        let pair_address = self.0.pair_provider.pair_address(&pair);
        let pair_contract = ISwaprPair::at(&self.0.web3, pair_address);

        let fetch_pool = self.0.read_state(pair, block);
        let fetch_fee = pair_contract.swap_fee().block(block).call();

        async move {
            let (pool, fee) = futures::join!(fetch_pool, fetch_fee);
            handle_results(pool, fee)
        }
        .boxed()
    }
}

fn handle_results(
    pool: Result<Option<Pool>>,
    fee: Result<u32, MethodError>,
) -> Result<Option<Pool>> {
    let fee = pool_fetching::handle_contract_error(fee)?;
    Ok(pool?.and_then(|pool| {
        Some(Pool {
            fee: Ratio::new(fee?, FEE_BASE),
            ..pool
        })
    }))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            ethrpc::{create_env_test_transport, Web3},
            recent_block_cache::Block,
            sources::{uniswap_v2, BaselineSource},
        },
        contracts::errors::testing_contract_error,
        ethcontract::H160,
        maplit::hashset,
    };

    #[test]
    fn sets_fee() {
        let tokens = TokenPair::new(H160([1; 20]), H160([2; 20])).unwrap();
        let address = H160::from_low_u64_be(1);
        assert_eq!(
            handle_results(
                Ok(Some(Pool {
                    address,
                    tokens,
                    reserves: (13, 37),
                    fee: Ratio::new(3, 1000),
                })),
                Ok(42),
            )
            .unwrap()
            .unwrap(),
            Pool {
                address,
                tokens,
                reserves: (13, 37),
                fee: Ratio::new(42, 10000),
            }
        );
    }

    #[test]
    fn ignores_contract_errors_when_reading_fee() {
        let tokens = TokenPair::new(H160([1; 20]), H160([2; 20])).unwrap();
        let address = H160::from_low_u64_be(1);
        assert!(handle_results(
            Ok(Some(Pool::uniswap(address, tokens, (0, 0)))),
            Err(testing_contract_error()),
        )
        .unwrap()
        .is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_swapr_pool() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let version = web3.eth().chain_id().await.unwrap().to_string();
        let pool_fetcher = uniswap_v2::UniV2BaselineSourceParameters::from_baseline_source(
            BaselineSource::Swapr,
            &version,
        )
        .unwrap()
        .into_source(&web3)
        .await
        .unwrap()
        .pool_fetching;
        let pool = pool_fetcher
            .fetch(
                hashset! {
                    TokenPair::new(
                        addr!("6A023CCd1ff6F2045C3309768eAd9E68F978f6e1"),
                        addr!("e91D153E0b41518A2Ce8Dd3D7944Fa863463a97d"),
                    )
                    .unwrap(),
                },
                Block::Recent,
            )
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        println!("WETH <> wxDAI pool: {pool:#?}");
    }
}

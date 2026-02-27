//! A pool state reading implementation specific to Swapr.

use {
    crate::sources::uniswap_v2::pool_fetching::{DefaultPoolReader, Pool, PoolReading},
    alloy::eips::BlockId,
    anyhow::Result,
    contracts::ISwaprPair,
    ethrpc::alloy::errors::ignore_non_node_error,
    futures::{FutureExt as _, future::BoxFuture},
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
        let fetch_pool = self.0.read_state(pair, block);

        async move {
            let pair_contract =
                ISwaprPair::Instance::new(pair_address, self.0.web3.provider.clone());
            let fetch_fee = pair_contract.swapFee().block(block);

            let (pool, fee) = futures::join!(fetch_pool, fetch_fee.call().into_future());
            handle_results(pool, fee)
        }
        .boxed()
    }
}

fn handle_results(
    pool: Result<Option<Pool>>,
    fee: Result<u32, alloy::contract::Error>,
) -> Result<Option<Pool>> {
    let fee = ignore_non_node_error(fee)?;
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
            recent_block_cache::Block,
            sources::{BaselineSource, uniswap_v2},
            web3::Web3,
        },
        alloy::{
            primitives::{Address, address},
            providers::Provider,
        },
        ethrpc::alloy::errors::testing_alloy_contract_error,
        maplit::hashset,
    };

    #[test]
    fn sets_fee() {
        let tokens =
            TokenPair::new(Address::from_slice(&[1; 20]), Address::from_slice(&[2; 20])).unwrap();
        let address = Address::with_last_byte(1);
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
        let tokens =
            TokenPair::new(Address::from_slice(&[1; 20]), Address::from_slice(&[2; 20])).unwrap();
        let address = Address::with_last_byte(1);
        assert!(
            handle_results(
                Ok(Some(Pool::uniswap(address, tokens, (0, 0)))),
                Err(testing_alloy_contract_error()),
            )
            .unwrap()
            .is_none()
        );
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_swapr_pool() {
        let web3 = Web3::new_from_env();
        let version = web3.provider.get_chain_id().await.unwrap().to_string();
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
                        address!("6A023CCd1ff6F2045C3309768eAd9E68F978f6e1"),
                        address!("e91D153E0b41518A2Ce8Dd3D7944Fa863463a97d"),
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

//! A pool state reading implementation specific to Swapr.

use {
    crate::sources::uniswap_v2::pool_fetching::{DefaultPoolReader, Pool, PoolReading},
    alloy::sol_types::GenericContractError,
    anyhow::Result,
    contracts::alloy::ISwaprPair,
    ethcontract::BlockId,
    ethrpc::alloy::conversions::IntoAlloy,
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
                ISwaprPair::Instance::new(pair_address.into_alloy(), self.0.web3.alloy.clone());
            let fetch_fee = pair_contract.swapFee().block(block.into_alloy());

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
    use alloy::contract::Error;
    let fee = match fee {
        Ok(fee) => Some(fee),
        // Alloy "hides" the contract execution errors under the transport error
        Err(err @ Error::TransportError(_)) => {
            // So we need to try to decode the error as a generic contract error,
            // we return in case it isn't a contract error
            match err.try_decode_into_interface_error::<GenericContractError>() {
                Ok(_) => None, // contract error
                Err(err) => return Err(err)?,
            }
        }
        Err(_) => None,
    };
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
            ethrpc::Web3,
            recent_block_cache::Block,
            sources::{BaselineSource, uniswap_v2},
        },
        contracts::errors::testing_alloy_contract_error,
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

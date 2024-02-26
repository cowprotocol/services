use {
    super::{BlockInfo, BlockNumberHash, BlockRetrieving, RangeInclusive},
    crate::Web3,
    anyhow::{bail, Context, Result},
    contracts::support::FetchBlock,
    primitive_types::{H256, U256},
    web3::{
        transports::Batch,
        types::{BlockNumber, CallRequest},
    },
};

/// A hybrid `eth_getBlock` and `eth_call` based block fetcher.
///
/// This is similar to the `eth_call` based fetcher, in that it can be used
/// for nodes where `eth_getBlockBy*` and `eth_blockNumber` calls return the
/// latest block for which a header is available even if the state isn't.
///
/// However, some nodes (notably Nethermind) do **not** support `eth_call` on
/// the pending block, which is required for the `eth_call` based fetcher to
/// work. As a work-around, we issue simultaneous `eth_call` and `eth_getBlock`
/// requests to fetch the full block header (which includes the hash) and
/// simulate code on the latest block for which there is state. This gives us
/// the best of both worlds at the cost of an extra request per "poll".
#[derive(Debug)]
pub struct BlockRetriever(pub Web3);

#[async_trait::async_trait]
impl BlockRetrieving for BlockRetriever {
    async fn current_block(&self) -> Result<BlockInfo> {
        let (return_data, block) = {
            let batch = web3::Web3::new(Batch::new(self.0.transport().clone()));

            let bytecode = <FetchBlock>::raw_contract().bytecode.to_bytes().unwrap();
            let return_data = batch.eth().call(
                CallRequest {
                    data: Some(bytecode),
                    ..Default::default()
                },
                Some(BlockNumber::Latest.into()),
            );
            let block = batch.eth().block(BlockNumber::Latest.into());

            batch.transport().submit_batch().await?;

            (
                return_data.await?.0,
                block.await?.context("missing latest block")?,
            )
        };

        let call = decode(
            return_data
                .as_slice()
                .try_into()
                .context("unexpected block fetch return length")?,
        )?;
        let fetch = BlockInfo::try_from(block)?;

        // The `FetchBlock` contract works by returning `block.number - 1`, its
        // hash, and its parent's hash. This means that, if we call it with
        // `latest`, then `call.number` will be the block `latest - 1`.
        //
        // We accept a few cases here:
        // 1. If `call.number + 1 >= fetch.number`, this means that the state for the
        //    `fetch.number` block is available (as the `eth_call` executed on that
        //    block or later). Hence, `Ok(fetch)` is the current block.
        // 2. If `call.number + 1 == fetch.number - 1`, then there is a 2 block
        //    differential between `call` and `fetch`, meaning that the `fetch.number`
        //    block header is available but not its state, so return the `fetch.number -
        //    1` as the current block.
        // 3. Unexpectedly large differential between `call` and `fetch`.
        if call.number.saturating_add(1) >= fetch.number {
            Ok(fetch)
        } else if call.number.saturating_add(1) == fetch.number.saturating_sub(1) {
            Ok(BlockInfo {
                number: fetch.number.saturating_sub(1),
                hash: fetch.parent_hash,
                timestamp: fetch.timestamp,
                parent_hash: call.hash,
                gas_limit: fetch.gas_limit,
            })
        } else {
            bail!("large differential between eth_getBlock {fetch:?} and eth_call {call:?}");
        }
    }

    async fn block(&self, number: u64) -> Result<BlockNumberHash> {
        self.0.block(number).await
    }

    async fn blocks(&self, range: RangeInclusive<u64>) -> Result<Vec<BlockNumberHash>> {
        self.0.blocks(range).await
    }
}

/// Decodes the return data from the `FetchBlock` contract.
fn decode(return_data: [u8; 160]) -> Result<BlockInfo> {
    let number = u64::try_from(U256::from_big_endian(&return_data[0..32]))
        .ok()
        .context("block number overflows u64")?;
    let hash = H256::from_slice(&return_data[32..64]);
    let parent_hash = H256::from_slice(&return_data[64..96]);
    let timestamp = U256::from_big_endian(&return_data[96..128]).as_u64();
    let gas_limit = U256::from_big_endian(&return_data[128..160]);

    Ok(BlockInfo {
        number,
        hash,
        parent_hash,
        timestamp,
        gas_limit,
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::create_env_test_transport};

    #[ignore]
    #[tokio::test]
    async fn node() {
        let retriever = BlockRetriever(Web3::new(create_env_test_transport()));

        let mut block = Option::<u64>::None;
        for _ in 0..3 {
            loop {
                let current = retriever.current_block().await.unwrap();
                if block.is_none() || matches!(block, Some(b) if b < current.number) {
                    println!("current block: {current:#?}");
                    block = Some(current.number);
                    break;
                }
            }
        }
    }
}

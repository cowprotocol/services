#[cfg(test)]
pub mod dummy_web3;
mod uniswap;

use crate::encoding;
use anyhow::anyhow;
use anyhow::Result;
use primitive_types::H160;
pub use uniswap::UniswapInteraction;

fn encode_interaction(
    target: H160,
    calldata: Vec<u8>,
    writer: &mut dyn std::io::Write,
) -> Result<()> {
    writer.write_all(target.as_fixed_bytes())?;
    writer.write_all(
        &encoding::encode_interaction_data_length(calldata.len())
            .ok_or_else(|| anyhow!("interaction data too long"))?,
    )?;
    writer.write_all(calldata.as_slice())?;
    Ok(())
}

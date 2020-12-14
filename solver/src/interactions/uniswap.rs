use crate::{encoding, settlement::Interaction};
use anyhow::Result;
use contracts::UniswapV2Router02;
use primitive_types::{H160, U256};

#[derive(Debug)]
pub struct UniswapInteraction {
    pub contract: UniswapV2Router02,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub token_in: H160,
    pub token_out: H160,
    pub payout_to: H160,
}

impl Interaction for UniswapInteraction {
    fn encode(&self, writer: &mut dyn std::io::Write) -> Result<()> {
        let method = self.contract.swap_exact_tokens_for_tokens(
            self.amount_in,
            self.amount_out_min,
            vec![self.token_in, self.token_out],
            self.payout_to,
            U256::MAX,
        );
        let data = method.tx.data.expect("no calldata").0;
        writer.write_all(self.contract.address().as_fixed_bytes())?;
        // Unwrap because we know uniswap data size can be stored in 3 bytes.
        writer.write_all(&encoding::encode_interaction_data_length(data.len()).unwrap())?;
        writer.write_all(data.as_slice())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{tests::u8_as_32_bytes_be, INTERACTION_BASE_SIZE};
    use crate::interactions::dummy_web3;
    use std::io::Cursor;

    #[test]
    fn encode_uniswap_call_() {
        let amount_in = 5;
        let amount_out_min = 6;
        let token_in = 7;
        let token_out = 8;
        let payout_to = 9;
        let contract = UniswapV2Router02::at(&dummy_web3::dummy_web3(), H160::from_low_u64_be(4));
        let interaction = UniswapInteraction {
            contract: contract.clone(),
            amount_in: amount_in.into(),
            amount_out_min: amount_out_min.into(),
            token_in: H160::from_low_u64_be(token_in as u64),
            token_out: H160::from_low_u64_be(token_out as u64),
            payout_to: H160::from_low_u64_be(payout_to as u64),
        };
        let mut cursor = Cursor::new(Vec::new());
        interaction.encode(&mut cursor).unwrap();
        let encoded = cursor.into_inner();
        assert_eq!(&encoded[0..20], contract.address().as_fixed_bytes());
        assert_eq!(encoded[20..23], [0, 1, 4]);
        let call = &encoded[INTERACTION_BASE_SIZE..];
        let signature = [0x38u8, 0xed, 0x17, 0x39];
        let path_offset = 160;
        let path_size = 2;
        let deadline = [0xffu8; 32];
        assert_eq!(call[0..4], signature);
        assert_eq!(call[4..36], u8_as_32_bytes_be(amount_in));
        assert_eq!(call[36..68], u8_as_32_bytes_be(amount_out_min));
        assert_eq!(call[68..100], u8_as_32_bytes_be(path_offset));
        assert_eq!(call[100..132], u8_as_32_bytes_be(payout_to));
        assert_eq!(call[132..164], deadline);
        assert_eq!(call[164..196], u8_as_32_bytes_be(path_size));
        assert_eq!(call[196..228], u8_as_32_bytes_be(token_in));
        assert_eq!(call[228..260], u8_as_32_bytes_be(token_out));
    }
}

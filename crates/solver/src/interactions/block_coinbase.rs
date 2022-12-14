use hex_literal::hex;
use primitive_types::{H160, U256};
use shared::interaction::{EncodedInteraction, Interaction};

// vk: A simple contract I made with verified code on etherscan:
// https://etherscan.io/address/0x5c2cD95CF750B8f8A4881d96F04bf571A07042B1
// Gas use for a full transaction when amount is 0 is 23849 and nonzero 30549.
const MAINNET_ADDRESS: H160 = H160(hex!("5c2cd95cf750b8f8a4881d96f04bf571a07042b1"));
const METHOD_ID: [u8; 4] = hex!("2755cd2d");

#[derive(Clone, Debug)]
pub struct PayBlockCoinbase {
    // ether wei
    pub amount: U256,
}

impl Interaction for PayBlockCoinbase {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(
            MAINNET_ADDRESS,
            self.amount,
            ethcontract::Bytes(METHOD_ID.to_vec()),
        )]
    }
}

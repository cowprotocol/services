use {
    alloy::{primitives::Address, sol_types::SolCall},
    contracts::alloy::UniswapV3SwapRouterV2::{
        IV3SwapRouter::ExactOutputSingleParams,
        UniswapV3SwapRouterV2::exactOutputSingleCall,
    },
    ethcontract::Bytes,
    ethrpc::alloy::conversions::IntoLegacy,
    shared::interaction::{EncodedInteraction, Interaction},
};

#[derive(Debug)]
pub struct UniswapV3Interaction {
    pub router: Address,
    pub params: ExactOutputSingleParams,
}

impl Interaction for UniswapV3Interaction {
    fn encode(&self) -> EncodedInteraction {
        (
            self.router.into_legacy(),
            0.into(),
            Bytes(
                exactOutputSingleCall {
                    params: self.params.clone(),
                }
                .abi_encode(),
            ),
        )
    }
}

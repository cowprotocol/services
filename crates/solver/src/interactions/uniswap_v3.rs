use {
    alloy::{
        primitives::{Address, U256},
        sol_types::SolCall,
    },
    contracts::UniswapV3SwapRouterV2::{
        IV3SwapRouter::ExactOutputSingleParams, UniswapV3SwapRouterV2::exactOutputSingleCall,
    },
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
            self.router,
            U256::ZERO,
            exactOutputSingleCall {
                params: self.params.clone(),
            }
            .abi_encode()
            .into(),
        )
    }
}

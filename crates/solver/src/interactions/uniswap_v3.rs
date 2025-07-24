use {
    contracts::UniswapV3SwapRouterV2,
    ethcontract::Bytes,
    primitive_types::{H160, U256},
    shared::{
        http_solver::model::TokenAmount,
        interaction::{EncodedInteraction, Interaction},
    },
};

#[derive(Debug)]
pub struct UniswapV3Interaction {
    pub router: UniswapV3SwapRouterV2,
    pub params: ExactOutputSingleParams,
}

#[derive(Debug)]
pub struct ExactOutputSingleParams {
    pub token_amount_in_max: TokenAmount,
    pub token_amount_out: TokenAmount,
    pub fee: u32,
    pub recipient: H160,
    pub sqrt_price_limit_x96: U256,
}
impl Interaction for UniswapV3Interaction {
    fn encode(&self) -> EncodedInteraction {
        let method = self.router.exact_output_single((
            self.params.token_amount_in_max.token,
            self.params.token_amount_out.token,
            self.params.fee,
            self.params.recipient,
            self.params.token_amount_out.amount,
            self.params.token_amount_in_max.amount,
            self.params.sqrt_price_limit_x96,
        ));
        let calldata = method.tx.data.expect("no calldata").0;
        (self.router.address(), 0.into(), Bytes(calldata))
    }
}

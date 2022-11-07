//! Module for abstracting the generation of calldata for roundtripping a token
//! in order to determine whether or not it is valid.

use crate::{
    ethrpc::extensions::{StateOverride, StateOverrides},
    trade_finding::{convert_interactions, Interaction},
};
use anyhow::Result;
use contracts::support::Trader;
use ethcontract::{H160, U256};
use std::{collections::HashMap, iter};
use web3::types::CallRequest;

/// A trait for abstracting roundtripping of a token from and to native token.
#[async_trait::async_trait]
pub trait RoundtripBuilding {
    async fn build_call(&self, token: H160) -> Result<Roundtrip>;
}

/// Data for a token roundtrip that can be simulated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Roundtrip {
    pub native: H160,
    pub token: H160,
    pub amount_native: U256,
    pub amount_token: U256,
    pub native_2_token: [Vec<Interaction>; 3],
    pub token_2_native: [Vec<Interaction>; 3],
    pub code_overrides: HashMap<H160, Vec<u8>>,
}

impl Roundtrip {
    /// Builds the calldata for the roundtrip.
    pub fn to_call(&self, trader: H160) -> (CallRequest, StateOverrides) {
        let trader = dummy_contract!(Trader, trader);
        let tx = trader
            .methods()
            .roundtrip(
                self.native,
                self.token,
                self.amount_token,
                convert_interactions(&self.native_2_token),
                convert_interactions(&self.token_2_native),
            )
            .tx;

        let state_overrides = iter::once((
            trader.address(),
            StateOverride {
                code: Some(deployed_bytecode!(Trader)),
                balance: Some(self.amount_native),
                ..Default::default()
            },
        ))
        .chain(self.code_overrides.iter().map(|(address, code)| {
            (
                *address,
                StateOverride {
                    code: Some(web3::types::Bytes(code.clone())),
                    ..Default::default()
                },
            )
        }))
        .collect();

        (
            CallRequest {
                to: tx.to,
                value: tx.value,
                data: tx.data,
                ..Default::default()
            },
            state_overrides,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{UniswapV2Router02, ERC20, WETH9};
    use maplit::hashmap;

    #[test]
    fn roundtrip_abi_encoding() {
        let settlement = addr!("9008D19f58AAbD9eD0D60971565AA8510560ab41");
        let uniswap_router = dummy_contract!(
            UniswapV2Router02,
            addr!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
        );

        let weth = dummy_contract!(WETH9, addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
        let usdc = dummy_contract!(ERC20, addr!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"));
        let amount_native = U256::from(1_000_000_000_000_000_000_u128); // 1,0 ETH
        let amount_usdc = U256::from(1_000_000_000_u128); // 1.000,0 USDC

        let roundtrip = Roundtrip {
            native: weth.address(),
            token: usdc.address(),
            amount_native,
            amount_token: amount_usdc,
            native_2_token: [
                vec![Interaction::from_call(
                    weth.methods()
                        .approve(uniswap_router.address(), U256::max_value()),
                )],
                vec![Interaction::from_call(
                    uniswap_router.methods().swap_tokens_for_exact_tokens(
                        amount_usdc,   // amountOut
                        amount_native, // amountInMax
                        vec![weth.address(), usdc.address()],
                        settlement,
                        U256::max_value(), // deadline
                    ),
                )],
                vec![],
            ],
            token_2_native: [
                vec![Interaction::from_call(
                    usdc.methods()
                        .approve(uniswap_router.address(), U256::max_value()),
                )],
                vec![Interaction::from_call(
                    uniswap_router.methods().swap_exact_tokens_for_tokens(
                        amount_usdc,  // amountIn
                        U256::zero(), // amountOutMin
                        vec![usdc.address(), weth.address()],
                        settlement,
                        U256::max_value(), // deadline
                    ),
                )],
                vec![],
            ],
            code_overrides: Default::default(),
        };

        let trader = addr!("9A204e02DAdD8f5A89FC37E6F7627789615824Eb");
        let (call, overrides) = roundtrip.to_call(trader);

        assert_eq!(
            call,
            CallRequest {
                to: Some(trader),
                data: Some(bytes!(
                    "1997f448
                     000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
                     000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
                     000000000000000000000000000000000000000000000000000000003b9aca00
                     00000000000000000000000000000000000000000000000000000000000000a0
                     0000000000000000000000000000000000000000000000000000000000000420
                     0000000000000000000000000000000000000000000000000000000000000060
                     0000000000000000000000000000000000000000000000000000000000000180
                     0000000000000000000000000000000000000000000000000000000000000360
                     0000000000000000000000000000000000000000000000000000000000000001
                     0000000000000000000000000000000000000000000000000000000000000020
                     000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
                     0000000000000000000000000000000000000000000000000000000000000000
                     0000000000000000000000000000000000000000000000000000000000000060
                     0000000000000000000000000000000000000000000000000000000000000044
                     095ea7b3
                     0000000000000000000000007a250d5630b4cf539739df2c5dacb4c659f2488d
                     ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
                             00000000000000000000000000000000000000000000000000000000
                     0000000000000000000000000000000000000000000000000000000000000001
                     0000000000000000000000000000000000000000000000000000000000000020
                     0000000000000000000000007a250d5630b4cf539739df2c5dacb4c659f2488d
                     0000000000000000000000000000000000000000000000000000000000000000
                     0000000000000000000000000000000000000000000000000000000000000060
                     0000000000000000000000000000000000000000000000000000000000000104
                     8803dbee
                     000000000000000000000000000000000000000000000000000000003b9aca00
                     0000000000000000000000000000000000000000000000000de0b6b3a7640000
                     00000000000000000000000000000000000000000000000000000000000000a0
                     0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41
                     ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
                     0000000000000000000000000000000000000000000000000000000000000002
                     000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
                     000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
                             00000000000000000000000000000000000000000000000000000000
                     0000000000000000000000000000000000000000000000000000000000000000
                     0000000000000000000000000000000000000000000000000000000000000060
                     0000000000000000000000000000000000000000000000000000000000000180
                     0000000000000000000000000000000000000000000000000000000000000360
                     0000000000000000000000000000000000000000000000000000000000000001
                     0000000000000000000000000000000000000000000000000000000000000020
                     000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
                     0000000000000000000000000000000000000000000000000000000000000000
                     0000000000000000000000000000000000000000000000000000000000000060
                     0000000000000000000000000000000000000000000000000000000000000044
                     095ea7b3
                     0000000000000000000000007a250d5630b4cf539739df2c5dacb4c659f2488d
                     ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
                             00000000000000000000000000000000000000000000000000000000
                     0000000000000000000000000000000000000000000000000000000000000001
                     0000000000000000000000000000000000000000000000000000000000000020
                     0000000000000000000000007a250d5630b4cf539739df2c5dacb4c659f2488d
                     0000000000000000000000000000000000000000000000000000000000000000
                     0000000000000000000000000000000000000000000000000000000000000060
                     0000000000000000000000000000000000000000000000000000000000000104
                     38ed1739
                     000000000000000000000000000000000000000000000000000000003b9aca00
                     0000000000000000000000000000000000000000000000000000000000000000
                     00000000000000000000000000000000000000000000000000000000000000a0
                     0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41
                     ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
                     0000000000000000000000000000000000000000000000000000000000000002
                     000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
                     000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
                     0000000000000000000000000000000000000000000000000000000000000000
                             00000000000000000000000000000000000000000000000000000000"
                )),
                ..Default::default()
            }
        );

        assert_eq!(
            overrides,
            hashmap! {
                trader => StateOverride {
                    code: Some(deployed_bytecode!(Trader)),
                    balance: Some(amount_native),
                    ..Default::default()
                },
            },
        );
    }
}

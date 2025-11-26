use {
    alloy::{
        primitives::{Address, U256, uint},
        sol,
        sol_types::SolCall,
    },
    anyhow::Result,
    contracts::alloy::IUniswapLikeRouter::{self, IUniswapLikeRouter::IUniswapLikeRouterInstance},
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    solvers_dto::{
        auction::{Auction, Class, Kind, Order},
        solution::{Asset, Fulfillment, Interaction, OrderUid, Solution, Trade},
    },
    std::collections::HashMap,
};

sol! {
    #[sol(rpc)]
    interface IERC4626 {
        function asset() external view returns (address);
        function convertToShares(uint256 assets) external view returns (uint256);
        function convertToAssets(uint256 shares) external view returns (uint256);
        function deposit(uint256 assets, address receiver) external returns (uint256 shares);
        function mint(uint256 shares, address receiver) external returns (uint256 assets);
        function withdraw(uint256 assets, address receiver, address owner) external returns (uint256 assets);
        function redeem(uint256 shares, address receiver, address owner) external returns (uint256 assets);
    }
}

pub struct EulerSolver {
    settlement: Address,
    uniswap_v2_router: IUniswapLikeRouterInstance<ethrpc::AlloyProvider>,
    provider: ethrpc::AlloyProvider,
}

impl EulerSolver {
    pub fn new(
        provider: ethrpc::AlloyProvider,
        settlement: Address,
        uniswap_v2_router: Address,
    ) -> Self {
        Self {
            settlement,
            uniswap_v2_router: IUniswapLikeRouter::IUniswapLikeRouter::new(
                uniswap_v2_router,
                provider.clone(),
            ),
            provider,
        }
    }

    pub async fn solve(&self, auction: &Auction) -> Result<Vec<(Solution, U256)>> {
        let mut solutions = Vec::new();

        for (index, order) in auction.orders.iter().enumerate() {
            match self.solve_order(order, index as u64).await {
                Ok(result) => solutions.push(result),
                Err(err) => {
                    tracing::error!("Failed to generate a solution for an auction: {}", err)
                }
            }
        }

        Ok(solutions)
    }

    async fn solve_order(&self, order: &Order, solution_id: u64) -> Result<(Solution, U256)> {
        tracing::debug!(
            "Solving order: sell_token={:?}, buy_token={:?}, sell_amount={}, buy_amount={}",
            order.sell_token,
            order.buy_token,
            order.sell_amount,
            order.buy_amount
        );

        // Step 1: Unwrap the underlyingmost token
        let mut sell_underlying_tokens = vec![order.sell_token.into_alloy()];
        let mut buy_underlying_tokens = vec![order.buy_token.into_alloy()];

        while let Ok(new_underlying) =
            IERC4626::new(*sell_underlying_tokens.last().unwrap(), &self.provider)
                .asset()
                .call()
                .await
        {
            sell_underlying_tokens.push(new_underlying);
        }
        while let Ok(new_underlying) =
            IERC4626::new(*buy_underlying_tokens.last().unwrap(), &self.provider)
                .asset()
                .call()
                .await
        {
            buy_underlying_tokens.push(new_underlying);
        }

        tracing::debug!(
            "Discovered Underlying Tokens: {:?} {:?}",
            sell_underlying_tokens,
            buy_underlying_tokens
        );

        // Begin building interactions
        let mut interactions = Vec::new();

        // Add unwrap interaction if sell token is a vault
        let mut orig_sell_amount = order.sell_amount.into_alloy();
        if orig_sell_amount == uint!(22300745198530623141535718272648361505980416_U256) {
            orig_sell_amount = uint!(100000000000000000_U256);
        }

        let mut sell_amount = orig_sell_amount;
        for (index, token) in sell_underlying_tokens[0..sell_underlying_tokens.len() - 1]
            .iter()
            .enumerate()
        {
            let withdraw_calldata = IERC4626::redeemCall {
                shares: sell_amount,
                receiver: self.settlement,
                owner: self.settlement,
            }
            .abi_encode();

            tracing::debug!("unwrapping sell amount: {} @ {}", sell_amount, token);

            let new_sell_amount = IERC4626::new(*token, &self.provider)
                .convertToAssets(sell_amount)
                .call()
                .await?;

            interactions.push(Interaction::Custom(
                solvers_dto::solution::CustomInteraction {
                    internalize: false,
                    target: token.into_legacy(),
                    value: U256::from(0).into_legacy(),
                    calldata: withdraw_calldata,
                    allowances: vec![],
                    inputs: vec![Asset {
                        token: token.into_legacy(),
                        amount: sell_amount.into_legacy(),
                    }],
                    outputs: vec![Asset {
                        token: sell_underlying_tokens[index + 1].into_legacy(),
                        amount: new_sell_amount.into_legacy(),
                    }],
                },
            ));

            sell_amount = new_sell_amount;
        }

        let mut buy_amount =
            if sell_underlying_tokens.last().unwrap() != buy_underlying_tokens.last().unwrap() {
                // quote and biuld the uniswap swap
                let output_amounts = self
                    .uniswap_v2_router
                    .getAmountsOut(
                        sell_amount,
                        vec![
                            *sell_underlying_tokens.last().unwrap(),
                            *buy_underlying_tokens.last().unwrap(),
                        ],
                    )
                    .call()
                    .await?;

                let output_amount = output_amounts.last().unwrap();

                tracing::debug!(
                    "Uniswap quote: input={}, output={}",
                    sell_amount,
                    output_amount
                );

                // Add Uniswap swap interaction
                // The swap sends tokens to the settlement contract which will handle the rest
                let swap_calldata =
                    IUniswapLikeRouter::IUniswapLikeRouter::swapExactTokensForTokensCall {
                        amountIn: sell_amount,
                        amountOutMin: *output_amount,
                        path: vec![
                            *sell_underlying_tokens.last().unwrap(),
                            *buy_underlying_tokens.last().unwrap(),
                        ],
                        to: self.settlement,
                        deadline: U256::from(i32::MAX),
                    }
                    .abi_encode();

                interactions.push(Interaction::Custom(
                    solvers_dto::solution::CustomInteraction {
                        internalize: false,
                        target: self.uniswap_v2_router.address().into_legacy(),
                        value: U256::from(0).into_legacy(),
                        calldata: swap_calldata,
                        allowances: vec![solvers_dto::solution::Allowance {
                            token: sell_underlying_tokens.last().unwrap().into_legacy(),
                            spender: self.uniswap_v2_router.address().into_legacy(),
                            amount: sell_amount.into_legacy(),
                        }],
                        inputs: vec![Asset {
                            token: sell_underlying_tokens.last().unwrap().into_legacy(),
                            amount: sell_amount.into_legacy(),
                        }],
                        outputs: vec![Asset {
                            token: buy_underlying_tokens.last().unwrap().into_legacy(),
                            amount: output_amount.into_legacy(),
                        }],
                    },
                ));

                *output_amount
            } else {
                sell_amount
            };

        // Add wrap interaction if buy token is a vault
        for (index, buy_underlying) in buy_underlying_tokens[0..buy_underlying_tokens.len() - 1]
            .iter()
            .enumerate()
            .rev()
        {
            let deposit_calldata = IERC4626::depositCall {
                assets: buy_amount,
                receiver: self.settlement,
            }
            .abi_encode();

            tracing::debug!("wrapping buy amount: {} @ {}", buy_amount, buy_underlying);

            let new_amount = IERC4626::new(*buy_underlying, &self.provider)
                .convertToShares(buy_amount)
                .call()
                .await?;

            interactions.push(Interaction::Custom(
                solvers_dto::solution::CustomInteraction {
                    internalize: false,
                    target: buy_underlying.into_legacy(),
                    value: U256::from(0).into_legacy(),
                    calldata: deposit_calldata,
                    allowances: vec![solvers_dto::solution::Allowance {
                        token: buy_underlying_tokens[index + 1].into_legacy(),
                        spender: buy_underlying.into_legacy(),
                        amount: buy_amount.into_legacy(),
                    }],
                    inputs: vec![Asset {
                        token: buy_underlying_tokens[index + 1].into_legacy(),
                        amount: buy_amount.into_legacy(),
                    }],
                    outputs: vec![Asset {
                        token: buy_underlying.into_legacy(),
                        amount: new_amount.into_legacy(), // Assuming 1:1 for simplicity
                    }],
                },
            ));

            buy_amount = new_amount;
        }

        tracing::debug!("Final computed buy amount {}", buy_amount);

        // Step 5: Calculate clearing prices
        let mut prices = HashMap::new();

        // Set price for sell token (in buy token terms)
        prices.insert(
            order.sell_token,
            uint!(1000000000000000000_U256).into_legacy(),
        );
        prices.insert(
            order.buy_token,
            (orig_sell_amount * uint!(1010000000000000000_U256) / buy_amount).into_legacy(),
        );

        // Step 6: Collect wrappers from order
        let wrappers: Vec<solvers_dto::solution::WrapperCall> = order
            .wrappers
            .iter()
            .map(|w| solvers_dto::solution::WrapperCall {
                address: w.address,
                data: w.data.clone(),
            })
            .collect();

        tracing::debug!(
            "Including {} wrapper(s) from order in solution",
            wrappers.len()
        );

        // Step 7: Build the solution
        let solution = Solution {
            id: solution_id,
            prices,
            trades: vec![Trade::Fulfillment(Fulfillment {
                order: OrderUid(order.uid),
                executed_amount: orig_sell_amount.into_legacy(),
                fee: if matches!(order.class, Class::Limit) {
                    Some(uint!(0_U256).into_legacy())
                } else {
                    None
                },
            })],
            pre_interactions: vec![],
            interactions,
            post_interactions: vec![],
            gas: Some(200_000 + 50_000 + 50_000), // Rough estimate (uniswap + wrap + unwrap)the
            // TODO: include the cost of executing wrapper.
            flashloans: None,
            wrappers,
        };

        Ok((solution, buy_amount))
    }
}

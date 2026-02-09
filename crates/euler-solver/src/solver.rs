use {
    alloy::{
        primitives::{Address, U256, uint},
        sol,
        sol_types::SolCall,
    },
    anyhow::Result,
    contracts::alloy::IUniswapLikeRouter::{self, IUniswapLikeRouter::IUniswapLikeRouterInstance},
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

    /// Discovers the chain of underlying EIP-4626 tokens for a given token by recursively
    /// calling the `asset()` function on ERC4626 vaults.
    async fn discover_underlying_tokens(&self, token: Address) -> Vec<Address> {
        let mut underlying_tokens = vec![token];

        while let Ok(new_underlying) =
            IERC4626::new(*underlying_tokens.last().unwrap(), &self.provider)
                .asset()
                .call()
                .await
        {
            underlying_tokens.push(new_underlying);
        }

        underlying_tokens
    }

    async fn solve_order(&self, order: &Order, solution_id: u64) -> Result<(Solution, U256)> {
        tracing::debug!(
            "Solving order: sell_token={:?}, buy_token={:?}, sell_amount={}, buy_amount={}, kind={:?}",
            order.sell_token,
            order.buy_token,
            order.sell_amount,
            order.buy_amount,
            order.kind
        );

        match order.kind {
            Kind::Sell => self.solve_order_sell(order, solution_id).await,
            Kind::Buy => self.solve_order_buy(order, solution_id).await,
        }
    }

    async fn solve_order_sell(&self, order: &Order, solution_id: u64) -> Result<(Solution, U256)> {
        tracing::debug!(
            "Solving SELL order: sell_token={:?}, buy_token={:?}, sell_amount={}, buy_amount={}",
            order.sell_token,
            order.buy_token,
            order.sell_amount,
            order.buy_amount
        );

        // Step 1: Discover underlying tokens
        let sell_underlying_tokens = self
            .discover_underlying_tokens(order.sell_token)
            .await;
        let buy_underlying_tokens = self
            .discover_underlying_tokens(order.buy_token)
            .await;

        tracing::debug!(
            "Discovered Underlying Tokens: {:?} {:?}",
            sell_underlying_tokens,
            buy_underlying_tokens
        );

        // Begin building interactions
        let mut interactions = Vec::new();

        // Add unwrap interaction if sell token is a vault
        let mut orig_sell_amount = order.sell_amount;
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
                    target: *token,
                    value: U256::from(0),
                    calldata: withdraw_calldata,
                    allowances: vec![],
                    inputs: vec![Asset {
                        token: *token,
                        amount: sell_amount,
                    }],
                    outputs: vec![Asset {
                        token: sell_underlying_tokens[index + 1],
                        amount: new_sell_amount,
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
                        target: *self.uniswap_v2_router.address(),
                        value: U256::from(0),
                        calldata: swap_calldata,
                        allowances: vec![solvers_dto::solution::Allowance {
                            token: *sell_underlying_tokens.last().unwrap(),
                            spender: *self.uniswap_v2_router.address(),
                            amount: sell_amount,
                        }],
                        inputs: vec![Asset {
                            token: *sell_underlying_tokens.last().unwrap(),
                            amount: sell_amount,
                        }],
                        outputs: vec![Asset {
                            token: *buy_underlying_tokens.last().unwrap(),
                            amount: *output_amount,
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
                    target: *buy_underlying,
                    value: U256::from(0),
                    calldata: deposit_calldata,
                    allowances: vec![solvers_dto::solution::Allowance {
                        token: buy_underlying_tokens[index + 1],
                        spender: *buy_underlying,
                        amount: buy_amount,
                    }],
                    inputs: vec![Asset {
                        token: buy_underlying_tokens[index + 1],
                        amount: buy_amount,
                    }],
                    outputs: vec![Asset {
                        token: *buy_underlying,
                        amount: new_amount, // Assuming 1:1 for simplicity
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
            uint!(1000000000000000000_U256),
        );
        prices.insert(
            order.buy_token,
            orig_sell_amount * uint!(1010000000000000000_U256) / buy_amount,
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
                executed_amount: orig_sell_amount,
                fee: if matches!(order.class, Class::Limit) {
                    Some(uint!(0_U256))
                } else {
                    None
                },
            })],
            pre_interactions: vec![],
            interactions,
            post_interactions: vec![],
            gas: Some(200_000 + 50_000 + 50_000), // Rough estimate (uniswap + wrap + unwrap)
            // TODO: include the cost of executing wrapper.
            flashloans: None,
            wrappers,
        };

        Ok((solution, buy_amount))
    }

    async fn solve_order_buy(&self, order: &Order, solution_id: u64) -> Result<(Solution, U256)> {
        tracing::debug!(
            "Solving BUY order: sell_token={:?}, buy_token={:?}, sell_amount={}, buy_amount={}",
            order.sell_token,
            order.buy_token,
            order.sell_amount,
            order.buy_amount
        );

        // Step 1: Discover underlying tokens
        let sell_underlying_tokens = self
            .discover_underlying_tokens(order.sell_token)
            .await;
        let buy_underlying_tokens = self
            .discover_underlying_tokens(order.buy_token)
            .await;

        tracing::debug!(
            "Discovered Underlying Tokens: {:?} {:?}",
            sell_underlying_tokens,
            buy_underlying_tokens
        );

        // Step 2: Work backwards from desired buy_amount
        let mut desired_buy_amount = order.buy_amount;

        // Handle magic number for unlimited amounts
        if desired_buy_amount == uint!(22300745198530623141535718272648361505980416_U256) {
            desired_buy_amount = uint!(100000000000000000_U256);
        }

        // Step 3: Calculate how much underlying buy token we need
        // Work backwards through the buy token wrapping chain to find required underlying assets
        // Store amounts at each level: [desired_buy_amount (token 0), assets_for_token_1, assets_for_token_2, ...]
        // Add 1bp slippage at each level to compound up
        let mut buy_amounts = vec![desired_buy_amount];

        for buy_token in buy_underlying_tokens[0..buy_underlying_tokens.len() - 1].iter() {
            let current_shares = *buy_amounts.last().unwrap();
            tracing::debug!(
                "Calculating assets needed to mint {} shares of token {}",
                current_shares,
                buy_token
            );

            let required_assets = IERC4626::new(*buy_token, &self.provider)
                .convertToAssets(current_shares)
                .call()
                .await?;

            // Add 1bp slippage: multiply by 10001/10000
            let required_assets_with_slippage =
                required_assets * uint!(10001_U256) / uint!(10000_U256);

            buy_amounts.push(required_assets_with_slippage);
        }

        let required_buy_underlying = *buy_amounts.last().unwrap();

        tracing::debug!(
            "Need {} of underlying buy token {:?}, amounts: {:?}",
            required_buy_underlying,
            buy_underlying_tokens.last().unwrap(),
            buy_amounts
        );

        // Step 4: Calculate how much underlying sell token we need (via Uniswap)
        let required_sell_underlying =
            if sell_underlying_tokens.last().unwrap() != buy_underlying_tokens.last().unwrap() {
                // Use getAmountsIn to work backwards from desired output
                let input_amounts = self
                    .uniswap_v2_router
                    .getAmountsIn(
                        required_buy_underlying,
                        vec![
                            *sell_underlying_tokens.last().unwrap(),
                            *buy_underlying_tokens.last().unwrap(),
                        ],
                    )
                    .call()
                    .await?;

                let required_input = input_amounts[0];

                tracing::debug!(
                    "Uniswap quote (backwards): output={}, required_input={}",
                    required_buy_underlying,
                    required_input
                );

                required_input
            } else {
                required_buy_underlying
            };

        // Step 5: Calculate how many sell token shares we need to burn
        // Work backwards through the sell token unwrapping chain
        // Store amounts at each level working backwards: [underlying_amount, shares_for_token_n-1, ..., shares_for_token_0]
        // Add 1bp slippage at each level to compound up
        let mut sell_amounts = vec![required_sell_underlying];

        for token in sell_underlying_tokens[0..sell_underlying_tokens.len() - 1]
            .iter()
            .rev()
        {
            let current_assets = *sell_amounts.last().unwrap();
            tracing::debug!(
                "Calculating shares needed to withdraw {} assets from token {}",
                current_assets,
                token
            );

            let required_shares = IERC4626::new(*token, &self.provider)
                .convertToShares(current_assets)
                .call()
                .await?;

            // Add 1bp slippage: multiply by 10001/10000
            let required_shares_with_slippage =
                required_shares * uint!(10001_U256) / uint!(10000_U256);

            sell_amounts.push(required_shares_with_slippage);
        }

        // Reverse to get amounts in execution order: [token_0_shares, token_1_assets, ..., underlying_assets]
        sell_amounts.reverse();

        let required_sell_amount = sell_amounts[0];

        tracing::debug!(
            "Need to sell {} of token {:?}, amounts: {:?}",
            required_sell_amount,
            order.sell_token,
            sell_amounts
        );

        // Step 6: Build interactions (in execution order)
        let mut interactions = Vec::new();

        // Unwrap sell tokens using withdraw for exact assets out
        for (index, token) in sell_underlying_tokens[0..sell_underlying_tokens.len() - 1]
            .iter()
            .enumerate()
        {
            let shares_in = sell_amounts[index];
            let assets_out = sell_amounts[index + 1];

            let withdraw_calldata = IERC4626::withdrawCall {
                assets: assets_out,
                receiver: self.settlement,
                owner: self.settlement,
            }
            .abi_encode();

            tracing::debug!(
                "Withdrawing {} assets from {} using {} shares",
                assets_out,
                token,
                shares_in
            );

            interactions.push(Interaction::Custom(
                solvers_dto::solution::CustomInteraction {
                    internalize: false,
                    target: *token,
                    value: U256::from(0),
                    calldata: withdraw_calldata,
                    allowances: vec![],
                    inputs: vec![Asset {
                        token: *token,
                        amount: shares_in,
                    }],
                    outputs: vec![Asset {
                        token: sell_underlying_tokens[index + 1],
                        amount: assets_out,
                    }],
                },
            ));
        }

        // Swap via Uniswap (if needed)
        if sell_underlying_tokens.last().unwrap() != buy_underlying_tokens.last().unwrap() {
            let output_amounts = self
                .uniswap_v2_router
                .getAmountsOut(
                    required_sell_underlying,
                    vec![
                        *sell_underlying_tokens.last().unwrap(),
                        *buy_underlying_tokens.last().unwrap(),
                    ],
                )
                .call()
                .await?;

            let output_amount = output_amounts.last().unwrap();

            tracing::debug!(
                "Swapping {} for {} via Uniswap",
                required_sell_underlying,
                output_amount
            );

            let swap_calldata =
                IUniswapLikeRouter::IUniswapLikeRouter::swapExactTokensForTokensCall {
                    amountIn: required_sell_underlying,
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
                    target: *self.uniswap_v2_router.address(),
                    value: U256::from(0),
                    calldata: swap_calldata,
                    allowances: vec![solvers_dto::solution::Allowance {
                        token: *sell_underlying_tokens.last().unwrap(),
                        spender: *self.uniswap_v2_router.address(),
                        amount: required_sell_underlying,
                    }],
                    inputs: vec![Asset {
                        token: *sell_underlying_tokens.last().unwrap(),
                        amount: required_sell_underlying,
                    }],
                    outputs: vec![Asset {
                        token: *buy_underlying_tokens.last().unwrap(),
                        amount: *output_amount,
                    }],
                },
            ));
        }

        // Wrap buy tokens using mint for exact shares out
        // Iterate in reverse order (from deepest underlying back to original token)
        let num_buy_vaults = buy_underlying_tokens.len() - 1;
        for i in (0..num_buy_vaults).rev() {
            let buy_token = buy_underlying_tokens[i];
            let shares_out = buy_amounts[i];
            let assets_in = buy_amounts[i + 1];

            let mint_calldata = IERC4626::mintCall {
                shares: shares_out,
                receiver: self.settlement,
            }
            .abi_encode();

            tracing::debug!(
                "Minting {} shares of {} using {} assets",
                shares_out,
                buy_token,
                assets_in
            );

            interactions.push(Interaction::Custom(
                solvers_dto::solution::CustomInteraction {
                    internalize: false,
                    target: buy_token,
                    value: U256::from(0),
                    calldata: mint_calldata,
                    allowances: vec![solvers_dto::solution::Allowance {
                        token: buy_underlying_tokens[i + 1],
                        spender: buy_token,
                        amount: assets_in,
                    }],
                    inputs: vec![Asset {
                        token: buy_underlying_tokens[i + 1],
                        amount: assets_in,
                    }],
                    outputs: vec![Asset {
                        token: buy_token,
                        amount: shares_out,
                    }],
                },
            ));
        }

        tracing::debug!(
            "Final amounts: sell={}, buy={}",
            required_sell_amount,
            desired_buy_amount
        );

        // Step 7: Calculate clearing prices
        let mut prices = HashMap::new();

        prices.insert(
            order.sell_token,
            uint!(1000000000000000000_U256),
        );
        prices.insert(
            order.buy_token,
            required_sell_amount * uint!(1010000000000000000_U256) / desired_buy_amount,
        );

        // Step 8: Collect wrappers from order
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

        // Step 9: Build the solution
        // For BUY orders, executed_amount is the buy amount
        let solution = Solution {
            id: solution_id,
            prices,
            trades: vec![Trade::Fulfillment(Fulfillment {
                order: OrderUid(order.uid),
                executed_amount: desired_buy_amount,
                fee: if matches!(order.class, Class::Limit) {
                    Some(uint!(0_U256))
                } else {
                    None
                },
            })],
            pre_interactions: vec![],
            interactions,
            post_interactions: vec![],
            gas: Some(200_000 + 50_000 + 50_000),
            flashloans: None,
            wrappers,
        };

        Ok((solution, desired_buy_amount))
    }
}

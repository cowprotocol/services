use {
    super::{error::Math, interaction::Liquidity, settlement, slippage, trade::ClearingPrices},
    crate::{
        domain::{
            competition::{
                self,
                order::{self, Partial},
            },
            eth::{self, Ether, allowance},
            liquidity,
        },
        infra::{self, solver::ManageNativeToken},
        util::{Bytes, conv::u256::U256Ext},
    },
    allowance::Allowance,
    itertools::Itertools,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid interaction: {0:?}")]
    InvalidInteractionExecution(competition::solution::interaction::Liquidity),
    #[error("missing auction id")]
    MissingAuctionId,
    #[error("invalid clearing price: {0:?}")]
    InvalidClearingPrice(eth::TokenAddress),
    #[error(transparent)]
    Math(#[from] Math),
    // TODO: remove when contracts are deployed everywhere
    #[error("flashloan support disabled")]
    FlashloanSupportDisabled,
    #[error("no flashloan helper configured for lender: {0}")]
    UnsupportedFlashloanLender(eth::H160),
}

pub fn tx(
    auction: &competition::Auction,
    solution: &super::Solution,
    contracts: &infra::blockchain::Contracts,
    approvals: impl Iterator<Item = eth::allowance::Approval>,
    internalization: settlement::Internalization,
    solver_native_token: ManageNativeToken,
) -> Result<eth::Tx, Error> {
    let mut tokens = Vec::with_capacity(solution.prices.len() + (solution.trades().len() * 2));
    let mut clearing_prices =
        Vec::with_capacity(solution.prices.len() + (solution.trades().len() * 2));
    let mut trades: Vec<Trade> = Vec::with_capacity(solution.trades().len());
    let mut pre_interactions = solution.pre_interactions.clone();
    let mut interactions =
        Vec::with_capacity(approvals.size_hint().0 + solution.interactions().len());
    let mut post_interactions = solution.post_interactions.clone();
    let mut native_unwrap = eth::TokenAmount(eth::U256::zero());

    // Encode uniform clearing price vector
    for (token, amount) in solution
        .clearing_prices()
        .into_iter()
        .sorted_by_cached_key(|(token, _amount)| *token)
    {
        tokens.push(token.into());
        clearing_prices.push(amount);
    }

    // Encode trades with custom clearing prices
    for trade in solution.trades() {
        let (price, mut trade) = match trade {
            super::Trade::Fulfillment(trade) => {
                pre_interactions.extend(trade.order().pre_interactions.clone());
                post_interactions.extend(trade.order().post_interactions.clone());

                let uniform_prices = ClearingPrices {
                    sell: solution
                        .clearing_price(trade.order().sell.token)
                        .ok_or(Error::InvalidClearingPrice(trade.order().sell.token))?,
                    buy: solution
                        .clearing_price(trade.order().buy.token)
                        .ok_or(Error::InvalidClearingPrice(trade.order().buy.token))?,
                };

                // Account for the WETH unwrap if necessary
                if trade.order().buy.token == eth::ETH_TOKEN {
                    native_unwrap += trade.buy_amount(&uniform_prices)?;
                }

                let custom_prices = trade.custom_prices(&uniform_prices)?;
                (
                    Price {
                        sell_token: trade.order().sell.token.into(),
                        sell_price: custom_prices.sell,
                        buy_token: trade.order().buy.token.into(),
                        buy_price: custom_prices.buy,
                    },
                    Trade {
                        // indices are set below
                        sell_token_index: Default::default(),
                        buy_token_index: Default::default(),
                        receiver: trade.order().receiver.unwrap_or_default().into(),
                        sell_amount: trade.order().sell.amount.into(),
                        buy_amount: trade.order().buy.amount.into(),
                        valid_to: trade.order().valid_to.into(),
                        app_data: trade.order().app_data.hash().0.0.into(),
                        fee_amount: eth::U256::zero(),
                        flags: Flags {
                            side: trade.order().side,
                            partially_fillable: matches!(
                                trade.order().partial,
                                Partial::Yes { .. }
                            ),
                            signing_scheme: trade.order().signature.scheme,
                            sell_token_balance: trade.order().sell_token_balance,
                            buy_token_balance: trade.order().buy_token_balance,
                        },
                        executed_amount: match trade.order().side {
                            order::Side::Sell => trade.executed().0 + trade.fee().0,
                            order::Side::Buy => trade.executed().into(),
                        },
                        signature: codec::signature(&trade.order().signature),
                    },
                )
            }
            super::Trade::Jit(trade) => {
                (
                    Price {
                        // Jit orders are matched at limit price, so the sell token is worth
                        // buy.amount and vice versa
                        sell_token: trade.order().sell.token.into(),
                        sell_price: trade.order().buy.amount.into(),
                        buy_token: trade.order().buy.token.into(),
                        buy_price: trade.order().sell.amount.into(),
                    },
                    Trade {
                        // indices are set below
                        sell_token_index: Default::default(),
                        buy_token_index: Default::default(),
                        receiver: trade.order().receiver.into(),
                        sell_amount: trade.order().sell.amount.into(),
                        buy_amount: trade.order().buy.amount.into(),
                        valid_to: trade.order().valid_to.into(),
                        app_data: trade.order().app_data.0.0.into(),
                        fee_amount: eth::U256::zero(),
                        flags: Flags {
                            side: trade.order().side,
                            partially_fillable: matches!(
                                trade.order().partially_fillable(),
                                order::Partial::Yes { .. }
                            ),
                            signing_scheme: trade.order().signature.scheme,
                            sell_token_balance: trade.order().sell_token_balance,
                            buy_token_balance: trade.order().buy_token_balance,
                        },
                        executed_amount: trade.executed().into(),
                        signature: codec::signature(&trade.order().signature),
                    },
                )
            }
        };
        tokens.push(price.sell_token);
        tokens.push(price.buy_token);
        clearing_prices.push(price.sell_price);
        clearing_prices.push(price.buy_price);

        trade.sell_token_index = (tokens.len() - 2).into();
        trade.buy_token_index = (tokens.len() - 1).into();

        trades.push(trade);
    }

    // Encode allowances
    for approval in approvals {
        interactions.push(approve(&approval.0))
    }

    // Encode interactions
    let slippage = slippage::Parameters {
        relative: solution.solver().slippage().relative.clone(),
        max: solution.solver().slippage().absolute.map(Ether::into),
        // TODO configure min slippage
        min: None,
        prices: auction.prices().clone(),
    };

    // Add all interactions needed to move flash loaned tokens around
    // These interactions are executed before all other pre-interactions
    let flashloans = solution
        .flashloans
        .iter()
        // Necessary pre-interactions get prepended to the settlement. So to initiate
        // the loans in the desired order we need to add them in reverse order.
        .rev()
        .map(|flashloan| {
            let flashloan_wrapper = contracts
                .get_flashloan_wrapper(&flashloan.lender)
                .ok_or(Error::UnsupportedFlashloanLender(flashloan.lender.0))?;

            // Allow settlement contract to pull borrowed tokens from flashloan wrapper
            pre_interactions.insert(
                0,
                approve_flashloan(
                    flashloan.token,
                    flashloan.amount,
                    contracts.settlement().address().into(),
                    &flashloan_wrapper.helper_contract,
                ),
            );

            // Transfer tokens from flashloan wrapper to user (i.e. borrower) to later allow
            // settlement contract to pull in all the necessary sell tokens from the user.
            let tx = contracts::ERC20::at(
                &contracts.settlement().raw_instance().web3(),
                flashloan.token.into(),
            )
            .transfer_from(
                flashloan_wrapper.helper_contract.address(),
                flashloan.borrower.into(),
                flashloan.amount.0,
            )
            .into_inner();
            pre_interactions.insert(
                1,
                eth::Interaction {
                    target: tx.to.unwrap().into(),
                    value: eth::U256::zero().into(),
                    call_data: tx.data.unwrap().0.into(),
                },
            );

            // Repayment amount needs to be increased by flash fee
            let fee_amount =
                (flashloan.amount.0 * flashloan_wrapper.fee_in_bps).ceil_div(&10_000.into());
            let repayment_amount = flashloan.amount.0 + fee_amount;

            // Since the order receiver is expected to be the setttlement contract, we need
            // to transfer tokens from the settlement contract to the flashloan wrapper
            let tx = contracts::ERC20::at(
                &contracts.settlement().raw_instance().web3(),
                flashloan.token.into(),
            )
            .transfer_from(
                contracts.settlement().address(),
                flashloan_wrapper.helper_contract.address(),
                repayment_amount,
            )
            .into_inner();
            post_interactions.push(eth::Interaction {
                target: tx.to.unwrap().into(),
                value: eth::U256::zero().into(),
                call_data: tx.data.unwrap().0.into(),
            });

            // Allow flash loan lender to take tokens from wrapper contract
            post_interactions.push(approve_flashloan(
                flashloan.token,
                repayment_amount.into(),
                flashloan.lender,
                &flashloan_wrapper.helper_contract,
            ));

            Ok((
                flashloan.amount.0,
                flashloan_wrapper.helper_contract.address(),
                flashloan.lender.0,
                flashloan.token.0.0,
            ))
        }).collect::<Result<Vec<(eth::U256, eth::H160, eth::H160, eth::H160)>, Error>>()?;

    for interaction in solution.interactions() {
        if matches!(internalization, settlement::Internalization::Enable)
            && interaction.internalize()
        {
            continue;
        }

        interactions.push(match interaction {
            competition::solution::Interaction::Custom(interaction) => eth::Interaction {
                value: interaction.value,
                target: interaction.target.into(),
                call_data: interaction.call_data.clone(),
            },
            competition::solution::Interaction::Liquidity(liquidity) => {
                liquidity_interaction(liquidity, &slippage, contracts.settlement())?
            }
        })
    }

    // Encode WETH unwrap
    if !native_unwrap.0.is_zero() && solver_native_token.insert_unwraps {
        interactions.push(unwrap(native_unwrap, contracts.weth()));
    }

    let tx = contracts
        .settlement()
        .settle(
            tokens,
            clearing_prices,
            trades.iter().map(codec::trade).collect(),
            [
                pre_interactions.iter().map(codec::interaction).collect(),
                interactions.iter().map(codec::interaction).collect(),
                post_interactions.iter().map(codec::interaction).collect(),
            ],
        )
        .into_inner();

    // Encode the auction id into the calldata
    let mut settle_calldata = tx.data.unwrap().0;
    settle_calldata.extend(auction.id().ok_or(Error::MissingAuctionId)?.to_be_bytes());

    // Target and calldata depend on whether a flashloan is used
    let (to, calldata) = if flashloans.is_empty() {
        (contracts.settlement().address().into(), settle_calldata)
    } else {
        let router = contracts
            .flashloan_router()
            .ok_or(Error::FlashloanSupportDisabled)?;
        let call = router.flash_loan_and_settle(flashloans, ethcontract::Bytes(settle_calldata));
        let calldata = call.tx.data.unwrap().0;
        (router.address().into(), calldata)
    };

    Ok(eth::Tx {
        from: solution.solver().address(),
        to,
        input: calldata.into(),
        value: Ether(0.into()),
        access_list: Default::default(),
    })
}

pub fn liquidity_interaction(
    liquidity: &Liquidity,
    slippage: &slippage::Parameters,
    settlement: &contracts::GPv2Settlement,
) -> Result<eth::Interaction, Error> {
    let (input, output) = slippage.apply_to(&slippage::Interaction {
        input: liquidity.input,
        output: liquidity.output,
    })?;

    match liquidity.liquidity.kind.clone() {
        liquidity::Kind::UniswapV2(pool) => pool
            .swap(&input, &output, &settlement.address().into())
            .ok(),
        liquidity::Kind::UniswapV3(pool) => pool
            .swap(&input, &output, &settlement.address().into())
            .ok(),
        liquidity::Kind::BalancerV2Stable(pool) => pool
            .swap(&input, &output, &settlement.address().into())
            .ok(),
        liquidity::Kind::BalancerV2Weighted(pool) => pool
            .swap(&input, &output, &settlement.address().into())
            .ok(),
        liquidity::Kind::Swapr(pool) => pool
            .swap(&input, &output, &settlement.address().into())
            .ok(),
        liquidity::Kind::ZeroEx(limit_order) => limit_order.to_interaction(&input).ok(),
    }
    .ok_or(Error::InvalidInteractionExecution(liquidity.clone()))
}

pub fn approve(allowance: &Allowance) -> eth::Interaction {
    let mut amount = [0u8; 32];
    let selector = hex_literal::hex!("095ea7b3");
    allowance.amount.to_big_endian(&mut amount);
    eth::Interaction {
        target: allowance.token.0.into(),
        value: eth::U256::zero().into(),
        // selector (4 bytes) + spender (20 byte address padded to 32 bytes) + amount (32 bytes)
        call_data: [
            selector.as_slice(),
            [0; 12].as_slice(),
            allowance.spender.0.as_bytes(),
            &amount,
        ]
        .concat()
        .into(),
    }
}

fn approve_flashloan(
    token: eth::TokenAddress,
    amount: eth::TokenAmount,
    spender: eth::ContractAddress,
    flashloan_wrapper: &contracts::IFlashLoanSolverWrapper,
) -> eth::Interaction {
    let tx = flashloan_wrapper
        .approve(token.into(), spender.into(), amount.0)
        .into_inner();
    eth::Interaction {
        target: tx.to.unwrap().into(),
        value: eth::U256::zero().into(),
        call_data: tx.data.unwrap().0.into(),
    }
}

fn unwrap(amount: eth::TokenAmount, weth: &contracts::WETH9) -> eth::Interaction {
    let tx = weth.withdraw(amount.into()).into_inner();
    eth::Interaction {
        target: tx.to.unwrap().into(),
        value: Ether(0.into()),
        call_data: tx.data.unwrap().0.into(),
    }
}

struct Trade {
    sell_token_index: eth::U256,
    buy_token_index: eth::U256,
    receiver: eth::H160,
    sell_amount: eth::U256,
    buy_amount: eth::U256,
    valid_to: u32,
    app_data: Bytes<[u8; 32]>,
    fee_amount: eth::U256,
    flags: Flags,
    executed_amount: eth::U256,
    signature: Bytes<Vec<u8>>,
}

struct Price {
    sell_token: eth::H160,
    sell_price: eth::U256,
    buy_token: eth::H160,
    buy_price: eth::U256,
}

struct Flags {
    side: order::Side,
    partially_fillable: bool,
    signing_scheme: order::signature::Scheme,
    sell_token_balance: order::SellTokenBalance,
    buy_token_balance: order::BuyTokenBalance,
}

pub mod codec {
    use crate::domain::{competition::order, eth};

    // cf. https://github.com/cowprotocol/contracts/blob/v1.5.0/src/contracts/libraries/GPv2Trade.sol#L16
    type Trade = (
        eth::U256,                    // sellTokenIndex
        eth::U256,                    // buyTokenIndex
        eth::H160,                    // receiver
        eth::U256,                    // sellAmount
        eth::U256,                    // buyAmount
        u32,                          // validTo
        ethcontract::Bytes<[u8; 32]>, // appData
        eth::U256,                    // feeAmount
        eth::U256,                    // flags
        eth::U256,                    // executedAmount
        ethcontract::Bytes<Vec<u8>>,  // signature
    );

    pub(super) fn trade(trade: &super::Trade) -> Trade {
        (
            trade.sell_token_index,
            trade.buy_token_index,
            trade.receiver,
            trade.sell_amount,
            trade.buy_amount,
            trade.valid_to,
            ethcontract::Bytes(trade.app_data.into()),
            trade.fee_amount,
            flags(&trade.flags),
            trade.executed_amount,
            ethcontract::Bytes(trade.signature.0.clone()),
        )
    }

    // cf. https://github.com/cowprotocol/contracts/blob/v1.5.0/src/contracts/libraries/GPv2Trade.sol#L58
    fn flags(flags: &super::Flags) -> eth::U256 {
        let mut result = 0u8;
        // The kind is encoded as 1 bit in position 0.
        result |= match flags.side {
            order::Side::Sell => 0b0,
            order::Side::Buy => 0b1,
        };
        // The order fill kind is encoded as 1 bit in position 1.
        result |= (flags.partially_fillable as u8) << 1;
        // The order sell token balance is encoded as 2 bits in position 2.
        result |= match flags.sell_token_balance {
            order::SellTokenBalance::Erc20 => 0b00,
            order::SellTokenBalance::External => 0b10,
            order::SellTokenBalance::Internal => 0b11,
        } << 2;
        // The order buy token balance is encoded as 1 bit in position 4.
        result |= match flags.buy_token_balance {
            order::BuyTokenBalance::Erc20 => 0b0,
            order::BuyTokenBalance::Internal => 0b1,
        } << 4;
        // The signing scheme is encoded as a 2 bits in position 5.
        result |= match flags.signing_scheme {
            order::signature::Scheme::Eip712 => 0b00,
            order::signature::Scheme::EthSign => 0b01,
            order::signature::Scheme::Eip1271 => 0b10,
            order::signature::Scheme::PreSign => 0b11,
        } << 5;
        result.into()
    }

    // cf. https://github.com/cowprotocol/contracts/blob/v1.5.0/src/contracts/libraries/GPv2Interaction.sol#L9
    type Interaction = (
        eth::H160,                   // target
        eth::U256,                   // value
        ethcontract::Bytes<Vec<u8>>, // signature
    );

    pub(super) fn interaction(interaction: &eth::Interaction) -> Interaction {
        (
            interaction.target.0,
            interaction.value.0,
            ethcontract::Bytes(interaction.call_data.0.clone()),
        )
    }

    pub fn signature(signature: &order::Signature) -> super::Bytes<Vec<u8>> {
        match signature.scheme {
            order::signature::Scheme::Eip712 | order::signature::Scheme::EthSign => {
                signature.data.clone()
            }
            order::signature::Scheme::Eip1271 => {
                super::Bytes([signature.signer.0.as_bytes(), signature.data.0.as_slice()].concat())
            }
            order::signature::Scheme::PreSign => {
                super::Bytes(signature.signer.0.as_bytes().to_vec())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, hex_literal::hex};

    #[test]
    fn test_approve() {
        let allowance = Allowance {
            token: eth::H160::from_slice(&hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")).into(),
            spender: eth::H160::from_slice(&hex!("000000000022D473030F116dDEE9F6B43aC78BA3"))
                .into(),
            amount: eth::U256::max_value(),
        };
        let interaction = approve(&allowance);
        assert_eq!(
            interaction.target,
            eth::H160::from_slice(&hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")).into(),
        );
        assert_eq!(interaction.call_data.0.as_slice(), hex!("095ea7b3000000000000000000000000000000000022d473030f116ddee9f6b43ac78ba3ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"));
    }
}

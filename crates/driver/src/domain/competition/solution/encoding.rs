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
    },
    allowance::Allowance,
    alloy::{
        primitives::{Address, Bytes, FixedBytes, U256},
        sol_types::SolCall,
    },
    contracts::alloy::{FlashLoanRouter::LoanRequest, WETH9},
    itertools::Itertools,
    num::Zero,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid interaction: {0:?}")]
    InvalidInteractionExecution(Box<competition::solution::interaction::Liquidity>),
    #[error("missing auction id")]
    MissingAuctionId,
    #[error("invalid clearing price: {0:?}")]
    InvalidClearingPrice(eth::TokenAddress),
    #[error(transparent)]
    Math(#[from] Math),
    // TODO: remove when contracts are deployed everywhere
    #[error("flashloan support disabled")]
    FlashloanSupportDisabled,
    #[error("both wrappers and flashloans cannot be encoded in the same auction")]
    FlashloanWrappersIncompatible,
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
    let mut native_unwrap = eth::TokenAmount(eth::U256::ZERO);

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
                        receiver: trade.order().receiver.unwrap_or_default(),
                        sell_amount: trade.order().sell.amount.into(),
                        buy_amount: trade.order().buy.amount.into(),
                        valid_to: trade.order().valid_to.into(),
                        app_data: trade.order().app_data.hash().0.0.into(),
                        fee_amount: eth::U256::ZERO,
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
                        receiver: trade.order().receiver,
                        sell_amount: trade.order().sell.amount.into(),
                        buy_amount: trade.order().buy.amount.into(),
                        valid_to: trade.order().valid_to.into(),
                        app_data: trade.order().app_data.0.0.into(),
                        fee_amount: eth::U256::ZERO,
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

        trade.sell_token_index = U256::from(tokens.len() - 2);
        trade.buy_token_index = U256::from(tokens.len() - 1);

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
        prices: auction.native_prices().clone(),
    };

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
                liquidity_interaction(liquidity, &slippage, contracts.settlement().address())?
            }
        })
    }

    // Encode WETH unwrap
    if !native_unwrap.0.is_zero() && solver_native_token.insert_unwraps {
        interactions.push(unwrap(native_unwrap, contracts.weth()));
    }

    let has_flashloans = !solution.flashloans.is_empty();
    let has_wrappers = !solution.wrappers.is_empty();

    // Encode the base settlement calldata
    let mut settle_calldata = contracts
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
        .calldata()
        .to_vec();

    // Append auction ID to settlement calldata
    settle_calldata.extend(auction.id().ok_or(Error::MissingAuctionId)?.to_be_bytes());

    let (to, calldata) = if has_flashloans && has_wrappers {
        return Err(Error::FlashloanWrappersIncompatible);
    } else if has_flashloans {
        encode_flashloan_settlement(solution, contracts, settle_calldata)?
    } else if has_wrappers {
        encode_wrapper_settlement(solution, settle_calldata)
    } else {
        (*contracts.settlement().address(), settle_calldata)
    };

    Ok(eth::Tx {
        from: solution.solver().address(),
        to,
        input: calldata.into(),
        value: Ether::zero(),
        access_list: Default::default(),
    })
}

/// Encodes a settlement transaction that uses flashloans.
///
/// Takes the base settlement calldata and wraps it in a flashLoanAndSettle call
/// to the flashloan router contract.
///
/// Returns (router_address, flashloan_calldata)
fn encode_flashloan_settlement(
    solution: &super::Solution,
    contracts: &infra::blockchain::Contracts,
    settle_calldata: Vec<u8>,
) -> Result<(eth::Address, Vec<u8>), Error> {
    // Get flashloan router contract
    let router = contracts
        .flashloan_router()
        .ok_or(Error::FlashloanSupportDisabled)?;

    // Convert flashloans to LoanRequest format
    let flashloans = solution
        .flashloans
        .values()
        .map(|flashloan| LoanRequest::Data {
            amount: flashloan.amount.0,
            borrower: flashloan.protocol_adapter.0,
            lender: flashloan.liquidity_provider.0,
            token: flashloan.token.0.0,
        })
        .collect();

    // Wrap settlement in flashLoanAndSettle call
    let calldata = router
        .flashLoanAndSettle(flashloans, settle_calldata.into())
        .calldata()
        .to_vec();

    Ok((*router.address(), calldata))
}

/// Encodes a settlement transaction that uses wrapper contracts.
///
/// Takes the base settlement calldata and wraps it in a wrappedSettleCall
/// with encoded wrapper metadata. Since wrappers are a chain, the wrapper
/// address to call is also processed by this function.
///
/// Returns (first_wrapper_address, wrapped_calldata)
fn encode_wrapper_settlement(
    solution: &super::Solution,
    settle_calldata: Vec<u8>,
) -> (eth::Address, Vec<u8>) {
    // Encode wrapper metadata
    let wrapper_data = encode_wrapper_data(&solution.wrappers);

    // Create wrappedSettleCall
    let calldata = contracts::alloy::ICowWrapper::ICowWrapper::wrappedSettleCall {
        settleData: settle_calldata.into(),
        wrapperData: wrapper_data.into(),
    }
    .abi_encode();

    (solution.wrappers[0].address, calldata)
}

/// Encodes wrapper metadata for wrapper settlement calls.
///
/// The format is:
/// - For wrappers after the first: 20 bytes (address)
/// - For each wrapper: 2 bytes (data length as u16 in native endian) + data
///
/// More information about wrapper encoding:
/// https://www.notion.so/cownation/Generalized-Wrapper-2798da5f04ca8095a2d4c56b9d17134e?source=copy_link#2858da5f04ca807980bbf7f845354120
///
/// Note: The first wrapper address is omitted from the encoded data since it's
/// already used as the transaction target.
fn encode_wrapper_data(wrappers: &[super::WrapperCall]) -> Vec<u8> {
    let mut wrapper_data = Vec::new();

    for (index, w) in wrappers.iter().enumerate() {
        // Skip first wrapper's address (it's the transaction target)
        if index != 0 {
            wrapper_data.extend(w.address.as_slice());
        }

        // Encode data length as u16 in native endian, then the data itself
        wrapper_data.extend((w.data.len() as u16).to_be_bytes().to_vec());
        wrapper_data.extend(w.data.clone());
    }

    wrapper_data
}

pub fn liquidity_interaction(
    liquidity: &Liquidity,
    slippage: &slippage::Parameters,
    settlement_contract: &Address,
) -> Result<eth::Interaction, Error> {
    let (input, output) = slippage.apply_to(&slippage::Interaction {
        input: liquidity.input,
        output: liquidity.output,
    })?;

    match liquidity.liquidity.kind.clone() {
        liquidity::Kind::UniswapV2(pool) => pool.swap(&input, &output, settlement_contract).ok(),
        liquidity::Kind::UniswapV3(pool) => pool.swap(&input, &output, settlement_contract).ok(),
        liquidity::Kind::BalancerV2Stable(pool) => {
            pool.swap(&input, &output, settlement_contract).ok()
        }
        liquidity::Kind::BalancerV2Weighted(pool) => {
            pool.swap(&input, &output, settlement_contract).ok()
        }
        liquidity::Kind::Swapr(pool) => pool.swap(&input, &output, settlement_contract).ok(),
        liquidity::Kind::ZeroEx(limit_order) => limit_order.to_interaction(&input).ok(),
    }
    .ok_or(Error::InvalidInteractionExecution(Box::new(
        liquidity.clone(),
    )))
}

pub fn approve(allowance: &Allowance) -> eth::Interaction {
    let selector = hex_literal::hex!("095ea7b3");
    let amount: [_; 32] = allowance.amount.to_be_bytes();
    eth::Interaction {
        target: allowance.token.0.into(),
        value: Ether::zero(),
        // selector (4 bytes) + spender (20 byte address padded to 32 bytes) + amount (32 bytes)
        call_data: [
            selector.as_slice(),
            [0; 12].as_slice(),
            allowance.spender.as_slice(),
            &amount,
        ]
        .concat()
        .into(),
    }
}

fn unwrap(amount: eth::TokenAmount, weth: &WETH9::Instance) -> eth::Interaction {
    eth::Interaction {
        target: *weth.address(),
        value: Ether::zero(),
        call_data: weth.withdraw(amount.0).calldata().to_vec().into(),
    }
}

struct Trade {
    sell_token_index: eth::U256,
    buy_token_index: eth::U256,
    receiver: eth::Address,
    sell_amount: eth::U256,
    buy_amount: eth::U256,
    valid_to: u32,
    app_data: FixedBytes<32>,
    fee_amount: eth::U256,
    flags: Flags,
    executed_amount: eth::U256,
    signature: Bytes,
}

struct Price {
    sell_token: eth::Address,
    sell_price: eth::U256,
    buy_token: eth::Address,
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
    use {
        crate::domain::{competition::order, eth},
        alloy::primitives::{Bytes, U256},
        contracts::alloy::GPv2Settlement,
    };

    pub(super) fn trade(trade: &super::Trade) -> GPv2Settlement::GPv2Trade::Data {
        GPv2Settlement::GPv2Trade::Data {
            sellTokenIndex: trade.sell_token_index,
            buyTokenIndex: trade.buy_token_index,
            receiver: trade.receiver,
            sellAmount: trade.sell_amount,
            buyAmount: trade.buy_amount,
            validTo: trade.valid_to,
            appData: trade.app_data.0.into(),
            feeAmount: trade.fee_amount,
            flags: flags(&trade.flags),
            executedAmount: trade.executed_amount,
            signature: trade.signature.0.clone().into(),
        }
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
        U256::from(result)
    }

    pub(super) fn interaction(
        interaction: &eth::Interaction,
    ) -> GPv2Settlement::GPv2Interaction::Data {
        GPv2Settlement::GPv2Interaction::Data {
            target: interaction.target,
            value: interaction.value.0,
            callData: interaction.call_data.0.clone().into(),
        }
    }

    pub fn signature(signature: &order::Signature) -> Bytes {
        match signature.scheme {
            order::signature::Scheme::Eip712 | order::signature::Scheme::EthSign => {
                signature.data.clone()
            }
            order::signature::Scheme::Eip1271 => [signature.signer.as_slice(), &signature.data]
                .concat()
                .into(),
            order::signature::Scheme::PreSign => signature.signer.to_vec().into(),
        }
    }
}

pub fn tx2(solution: &super::Solution) -> Result<(), Error> {
    let mut tokens = Vec::with_capacity(solution.prices.len() + (solution.trades().len() * 2));
    let mut clearing_prices =
        Vec::with_capacity(solution.prices.len() + (solution.trades().len() * 2));
    let mut trades: Vec<Trade> = Vec::with_capacity(solution.trades().len());
    let mut pre_interactions = solution.pre_interactions.clone();

    let mut post_interactions = solution.post_interactions.clone();
    let mut native_unwrap = eth::TokenAmount(eth::U256::ZERO);

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
                        receiver: trade.order().receiver.unwrap_or_default(),
                        sell_amount: trade.order().sell.amount.into(),
                        buy_amount: trade.order().buy.amount.into(),
                        valid_to: trade.order().valid_to.into(),
                        app_data: trade.order().app_data.hash().0.0.into(),
                        fee_amount: eth::U256::ZERO,
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
                        receiver: trade.order().receiver,
                        sell_amount: trade.order().sell.amount.into(),
                        buy_amount: trade.order().buy.amount.into(),
                        valid_to: trade.order().valid_to.into(),
                        app_data: trade.order().app_data.0.0.into(),
                        fee_amount: eth::U256::ZERO,
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

        trade.sell_token_index = U256::from(tokens.len() - 2);
        trade.buy_token_index = U256::from(tokens.len() - 1);

        trades.push(trade);
    }

    dbg!(tokens, clearing_prices);

    Ok(())
}


#[cfg(test)]
mod test {
    use {
        super::*,
        crate::{
            domain::{
                competition::{self, order},
                eth,
            },
            infra::config::file::FeeHandler,
        },
        alloy::primitives::address,
        hex_literal::hex,
        solvers_dto::solution::{Fulfillment as DtoFulfillment, Interaction, Solution as DtoSolution, Trade as DtoTrade},
        std::collections::HashSet,
    };

    fn convert_call(call: solvers_dto::solution::Call) -> eth::Interaction {
        eth::Interaction {
            target: call.target.into(),
            value: call.value.into(),
            call_data: call.calldata.into(),
        }
    }

    fn convert_interaction(interaction: Interaction) -> super::super::Interaction {
        match interaction {
            Interaction::Custom(interaction) => super::super::Interaction::Custom(
                super::super::interaction::Custom {
                    target: interaction.target.into(),
                    value: interaction.value.into(),
                    call_data: interaction.calldata.into(),
                    allowances: interaction
                        .allowances
                        .into_iter()
                        .map(|allowance| {
                            eth::Allowance {
                                token: allowance.token.into(),
                                spender: allowance.spender,
                                amount: allowance.amount,
                            }
                            .into()
                        })
                        .collect(),
                    inputs: interaction
                        .inputs
                        .into_iter()
                        .map(|asset| eth::Asset {
                            token: asset.token.into(),
                            amount: asset.amount.into(),
                        })
                        .collect(),
                    outputs: interaction
                        .outputs
                        .into_iter()
                        .map(|asset| eth::Asset {
                            token: asset.token.into(),
                            amount: asset.amount.into(),
                        })
                        .collect(),
                    internalize: interaction.internalize,
                },
            ),
            Interaction::Liquidity(_) => panic!("test helper does not support liquidity interactions"),
        }
    }

    fn fulfillment_assets(
        dto: &DtoSolution,
        trade: &DtoFulfillment,
    ) -> (eth::Asset, eth::Asset, order::Side) {
        let fee = trade.fee.unwrap_or_default();

        if let Some((input, output)) = dto.interactions.iter().find_map(|interaction| match interaction {
            Interaction::Custom(interaction) => Some((interaction.inputs.first()?.clone(), interaction.outputs.first()?.clone())),
            Interaction::Liquidity(_) => None,
        }) {
            return (
                eth::Asset {
                    token: input.token.into(),
                    amount: (trade.executed_amount + fee).into(),
                },
                eth::Asset {
                    token: output.token.into(),
                    amount: output.amount.into(),
                },
                order::Side::Sell,
            );
        }

        let mut prices = dto.prices.keys().copied();
        let sell_token = prices.next().expect("dto solution missing sell token");
        let buy_token = prices.next().unwrap_or(sell_token);
        (
            eth::Asset {
                token: sell_token.into(),
                amount: (trade.executed_amount + fee).into(),
            },
            eth::Asset {
                token: buy_token.into(),
                amount: trade.executed_amount.into(),
            },
            order::Side::Sell,
        )
    }

    fn convert_trade(dto: &DtoSolution, trade: DtoTrade) -> super::super::Trade {
        match trade {
            DtoTrade::Fulfillment(trade) => {
                let uid = competition::order::Uid::from(&trade.order);
                let owner = uid.owner();
                let fee = trade.fee.unwrap_or_default();
                let kind = if trade.fee.is_some() {
                    order::Kind::Limit
                } else {
                    order::Kind::Market
                };
                let (sell, buy, side) = fulfillment_assets(dto, &trade);

                super::super::Trade::Fulfillment(
                    super::super::trade::Fulfillment::new(
                        competition::Order {
                            uid,
                            receiver: Some(owner),
                            created: 0.into(),
                            valid_to: uid.valid_to().into(),
                            buy,
                            sell,
                            side,
                            kind,
                            app_data: Default::default(),
                            partial: order::Partial::No,
                            pre_interactions: vec![],
                            post_interactions: vec![],
                            sell_token_balance: order::SellTokenBalance::Erc20,
                            buy_token_balance: order::BuyTokenBalance::Erc20,
                            signature: order::Signature {
                                scheme: order::signature::Scheme::PreSign,
                                data: Default::default(),
                                signer: owner,
                            },
                            protocol_fees: vec![],
                            quote: None,
                        },
                        trade.executed_amount.into(),
                        match trade.fee {
                            Some(fee) => {
                                super::super::trade::Fee::Dynamic(order::SellAmount(fee))
                            }
                            None => super::super::trade::Fee::Static,
                        },
                        eth::U256::ZERO,
                    )
                    .expect("dto fulfillment should convert into a valid domain fulfillment"),
                )
            }
            DtoTrade::Jit(_) => panic!("test helper does not support jit trades"),
        }
    }

    fn convert_solution(dto: DtoSolution) -> super::super::Solution {
        let surplus_capturing_jit_order_owners = HashSet::new();
        super::super::Solution::new(
            super::super::Id::new(dto.id),
            dto.trades
                .clone()
                .into_iter()
                .map(|trade| convert_trade(&dto, trade))
                .collect(),
            dto.prices
                .into_iter()
                .map(|(token, price)| (token.into(), price))
                .collect(),
            dto.pre_interactions.into_iter().map(convert_call).collect(),
            dto.interactions.into_iter().map(convert_interaction).collect(),
            dto.post_interactions.into_iter().map(convert_call).collect(),
            super::super::SolverInfo::for_tests(address!("0000000000000000000000000000000000000007")),
            eth::WethAddress(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").into()),
            dto.gas.map(eth::Gas::from),
            FeeHandler::Driver,
            &surplus_capturing_jit_order_owners,
            dto.flashloans
                .unwrap_or_default()
                .iter()
                .map(|(uid, flashloan)| (competition::order::Uid::from(uid), flashloan.into()))
                .collect(),
            dto.wrappers
                .into_iter()
                .map(|wrapper| super::super::WrapperCall {
                    address: wrapper.address,
                    data: wrapper.data,
                })
                .collect(),
        )
        .expect("dto solution should convert into a valid domain solution")
    }

    #[test]
    fn foo() {
        let solution: DtoSolution = serde_json::from_str(
            r#"{"id":0,"prices":{"0xdac17f958d2ee523a2206206994597c13d831ec7":"10145969399","0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48":"10146325210"},"trades":[{"kind":"fulfillment","order":"0x58996c2299013175e448808628f74034c24e436840667e442a0c6d45a3aa735b9e3eb6b715aac6bb37fe891f205141d1f3afe10069cfd39a","executedAmount":"10145969399","fee":"0"}],"preInteractions":[],"interactions":[{"kind":"custom","internalize":false,"target":"0x179dc3fb0f2230094894317f307241a52cdb38aa","value":"0","callData":"0xf0d7bb940000000000000000000000002c0552e5dcb79b064fd23e358a86810bc5994244000000000000000000000000000000000000000000000000000000025cbf34f700000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000001600000000000000000000000000000000000000000000000000000000069cfd31900000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000025b8bed700000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab410000000000000000000000000000000000000000000000000000000000000001000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec700000000000000000000000000000000000000000000000000000000000000000000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab4100000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb480000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000086d34c17e54e1a96ad45fe7b49b52267118bd4f3000000000000000000000000000000000000000000000000000000000000008001020000002400000000000086d34c17e54e1a96ad45fe7b49b52267118bd4f300000000000000000000000000000000000000000000000000000000000002a40780c0670000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000025cbf34f700000000000000000000000055084ee0fef03f14a305cd24286359a35d73515100000000000000000000000000000000000000000000000000000000000000800000000000000000000000004c4c3e005c6cb9ce249a267f28299293a628cf380000000000000000000000006047b384d58dc7f8f6fef85d75754e6928f064840000000000000000000000002c0552e5dcb79b064fd23e358a86810bc599424400000000000000000000000055a37a2e5e5973510ac9d9c723aec213fa161919000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec70000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000025cbf34f7000000000000000000000000000000000000000000000000000000025cc4a2da0000000000000000000000000000000000000000000000000000000069cfd2db0000000000000000000000000000000000000000000000000000019d53cebd731f000000a0000a00000017482a9cc0ffffffffffffff002df567d5c5a59a000000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000000000000000000000000000000000000000000414d098bc161ebe39d51e7505a47cfaf64a32f7d58e9e31d7295505fcde6168d756ffe99a4d56c0702d832c17d2a47fc4504f3aae45d06fbc67b9ff9db623c4e161c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","allowances":[{"token":"0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48","spender":"0x179dc3fb0f2230094894317f307241a52cdb38aa","amount":"10145969399"}],"inputs":[{"token":"0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48","amount":"10145969399"}],"outputs":[{"token":"0xdac17f958d2ee523a2206206994597c13d831ec7","amount":"10146325210"}]}],"postInteractions":[],"bundleId":null,"stateOverrides":null}"#,
        )
        .unwrap();

        let solution = convert_solution(solution);

        assert_eq!(solution.trades().len(), 1);
        assert_eq!(solution.interactions().len(), 1);
        tx2(&solution).unwrap();
    }


    #[test]
    fn test_approve() {
        let allowance = Allowance {
            token: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").into(),
            spender: address!("000000000022D473030F116dDEE9F6B43aC78BA3"),
            amount: alloy::primitives::U256::MAX,
        };
        let interaction = approve(&allowance);
        assert_eq!(
            interaction.target,
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        );
        assert_eq!(
            interaction.call_data.as_ref(),
            hex!(
                "095ea7b3000000000000000000000000000000000022d473030f116ddee9f6b43ac78ba3ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            )
        );
    }
}

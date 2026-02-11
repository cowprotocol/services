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
        primitives::{Address, U256},
        sol_types::SolCall,
    },
    contracts::alloy::{FlashLoanRouter::LoanRequest, WETH9},
    itertools::Itertools,
    num::Zero,
    shared::bytes::Bytes,
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
    app_data: Bytes<[u8; 32]>,
    fee_amount: eth::U256,
    flags: Flags,
    executed_amount: eth::U256,
    signature: Bytes<Vec<u8>>,
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
        alloy::primitives::U256,
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

    pub fn signature(signature: &order::Signature) -> super::Bytes<Vec<u8>> {
        match signature.scheme {
            order::signature::Scheme::Eip712 | order::signature::Scheme::EthSign => {
                signature.data.clone()
            }
            order::signature::Scheme::Eip1271 => {
                [signature.signer.as_slice(), signature.data.0.as_slice()]
                    .concat()
                    .into()
            }
            order::signature::Scheme::PreSign => signature.signer.to_vec().into(),
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, alloy::primitives::address, hex_literal::hex};

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
            interaction.call_data.0.as_slice(),
            hex!(
                "095ea7b3000000000000000000000000000000000022d473030f116ddee9f6b43ac78ba3ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            )
        );
    }
}

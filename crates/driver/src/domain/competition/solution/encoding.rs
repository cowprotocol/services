use {
    super::{error::Math, settlement, trade::ClearingPrices},
    crate::{
        domain::{
            competition::{
                self,
                order::{self, Partial},
            },
            eth::{self, allowance, Ether},
            liquidity::{self, ExactOutput, MaxInput},
        },
        util::Bytes,
    },
    allowance::Allowance,
};

/// The type of strategy used to encode the solution.
#[derive(Debug, Copy, Clone)]
pub enum Strategy {
    /// Use logic from the legacy solver crate
    Boundary,
    /// Use logic from this module for encoding
    Domain,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid interaction: {0:?}")]
    InvalidInteractionExecution(competition::solution::interaction::Liquidity),
    #[error("invalid clearing price: {0:?}")]
    InvalidClearingPrice(eth::TokenAddress),
    #[error(transparent)]
    Math(#[from] Math),
}

pub fn tx(
    auction_id: competition::auction::Id,
    solution: &super::Solution,
    contract: &contracts::GPv2Settlement,
    approvals: impl Iterator<Item = eth::allowance::Approval>,
    internalization: settlement::Internalization,
) -> Result<eth::Tx, Error> {
    let mut tokens = Vec::new();
    let mut clearing_prices = Vec::new();
    let mut trades: Vec<Trade> = Vec::new();
    let mut pre_interactions = Vec::new();
    let mut interactions = Vec::new();
    let mut post_interactions = Vec::new();

    // Encode uniform clearing price vector
    for (token, price) in solution.prices.clone() {
        tokens.push(token.0.into());
        clearing_prices.push(price);
    }

    // Encode trades with custom clearing prices
    for trade in solution.trades() {
        match trade {
            super::Trade::Fulfillment(trade) => {
                tokens.push(trade.order().sell.token.0.into());
                tokens.push(trade.order().buy.token.0.into());

                let uniform_prices = ClearingPrices {
                    sell: solution
                        .clearing_price(trade.order().sell.token)
                        .ok_or(Error::InvalidClearingPrice(trade.order().sell.token))?,
                    buy: solution
                        .clearing_price(trade.order().buy.token)
                        .ok_or(Error::InvalidClearingPrice(trade.order().buy.token))?,
                };
                let custom_prices = trade.custom_prices(&uniform_prices)?;
                clearing_prices.push(custom_prices.sell);
                clearing_prices.push(custom_prices.buy);

                trades.push(Trade {
                    sell_token_index: (tokens.len() - 2).into(),
                    buy_token_index: (tokens.len() - 1).into(),
                    receiver: trade.order().receiver.unwrap_or_default().into(),
                    sell_amount: trade.order().sell.amount.into(),
                    buy_amount: trade.order().buy.amount.into(),
                    valid_to: trade.order().valid_to.0.into(),
                    app_data: trade.order().app_data.0 .0.into(),
                    fee_amount: eth::U256::zero(),
                    flags: order_flags(trade.order()),
                    executed_amount: trade.executed().0,
                    signature: trade.order().signature.data.clone(),
                });

                pre_interactions.extend(trade.order().pre_interactions.clone());
                post_interactions.extend(trade.order().post_interactions.clone());
            }
            super::Trade::Jit(trade) => {
                tokens.push(trade.order().sell.token.0.into());
                tokens.push(trade.order().buy.token.0.into());

                // Jit orders are matched at limit price, so the sell token is worth buy.amount
                // and vice versa
                clearing_prices.push(trade.order().buy.amount.into());
                clearing_prices.push(trade.order().sell.amount.into());

                trades.push(Trade {
                    sell_token_index: (tokens.len() - 2).into(),
                    buy_token_index: (tokens.len() - 1).into(),
                    receiver: trade.order().receiver.into(),
                    sell_amount: trade.order().sell.amount.into(),
                    buy_amount: trade.order().buy.amount.into(),
                    valid_to: trade.order().valid_to.0.into(),
                    app_data: trade.order().app_data.0 .0.into(),
                    fee_amount: eth::U256::zero(),
                    flags: jit_order_flags(trade.order()),
                    executed_amount: trade.executed().0,
                    signature: trade.order().signature.data.clone(),
                });
            }
        }
    }

    // Encode allowances
    for approval in approvals {
        interactions.push(approve(&approval.0))
    }

    // Encode interaction
    for interaction in solution.interactions() {
        if matches!(internalization, settlement::Internalization::Enable)
            && interaction.internalize()
        {
            continue;
        }

        interactions.push(match interaction {
            competition::solution::Interaction::Custom(interaction) => eth::Interaction {
                value: interaction.value.into(),
                target: interaction.target.0.into(),
                call_data: interaction.call_data.clone(),
            },
            competition::solution::Interaction::Liquidity(interaction) => {
                // Todo account for slippage
                let input = MaxInput(interaction.input);
                let output = ExactOutput(interaction.output);

                match interaction.liquidity.kind.clone() {
                    liquidity::Kind::UniswapV2(pool) => {
                        pool.swap(&input, &output, &contract.address().into()).ok()
                    }
                    liquidity::Kind::UniswapV3(pool) => {
                        pool.swap(&input, &output, &contract.address().into()).ok()
                    }
                    liquidity::Kind::BalancerV2Stable(pool) => {
                        pool.swap(&input, &output, &contract.address().into()).ok()
                    }
                    liquidity::Kind::BalancerV2Weighted(pool) => {
                        pool.swap(&input, &output, &contract.address().into()).ok()
                    }
                    liquidity::Kind::Swapr(pool) => {
                        pool.swap(&input, &output, &contract.address().into()).ok()
                    }
                    liquidity::Kind::ZeroEx(limit_order) => limit_order.to_interaction(&input).ok(),
                }
                .ok_or(Error::InvalidInteractionExecution(interaction.clone()))?
            }
        })
    }

    let tx = contract
        .settle(
            tokens.clone(),
            clearing_prices.clone(),
            trades.iter().map(codec::trade).collect(),
            [
                pre_interactions.iter().map(codec::interaction).collect(),
                interactions.iter().map(codec::interaction).collect(),
                post_interactions.iter().map(codec::interaction).collect(),
            ],
        )
        .into_inner();

    // Encode the auction id into the calldata
    let mut calldata = tx.data.unwrap().0;
    calldata.extend(auction_id.to_be_bytes());

    Ok(eth::Tx {
        from: solution.solver().address(),
        to: contract.address().into(),
        input: calldata.into(),
        value: Ether(0.into()),
        access_list: Default::default(),
    })
}

fn order_flags(order: &competition::Order) -> eth::U256 {
    let mut result = 0u8;
    // The kind is encoded as 1 bit in position 0.
    result |= match order.side {
        order::Side::Sell => 0b0,
        order::Side::Buy => 0b1,
    };
    // The order fill kind is encoded as 1 bit in position 1.
    result |= (matches!(order.partial, Partial::Yes { .. }) as u8) << 1;
    // The order sell token balance is encoded as 2 bits in position 2.
    result |= match order.sell_token_balance {
        order::SellTokenBalance::Erc20 => 0b00,
        order::SellTokenBalance::External => 0b10,
        order::SellTokenBalance::Internal => 0b11,
    } << 2;
    // The order buy token balance is encoded as 1 bit in position 4.
    result |= match order.buy_token_balance {
        order::BuyTokenBalance::Erc20 => 0b0,
        order::BuyTokenBalance::Internal => 0b1,
    } << 4;
    // The signing scheme is encoded as a 2 bits in position 5.
    result |= match order.signature.scheme {
        order::signature::Scheme::Eip712 => 0b00,
        order::signature::Scheme::EthSign => 0b01,
        order::signature::Scheme::Eip1271 => 0b10,
        order::signature::Scheme::PreSign => 0b11,
    } << 5;
    result.into()
}

fn jit_order_flags(order: &order::Jit) -> eth::U256 {
    let mut result = 0u8;
    // The kind is encoded as 1 bit in position 0.
    result |= match order.side {
        order::Side::Sell => 0b0,
        order::Side::Buy => 0b1,
    };
    // The order fill kind is encoded as 1 bit in position 1.
    result |= (order.partially_fillable as u8) << 1;
    // The order sell token balance is encoded as 2 bits in position 2.
    result |= match order.sell_token_balance {
        order::SellTokenBalance::Erc20 => 0b00,
        order::SellTokenBalance::External => 0b10,
        order::SellTokenBalance::Internal => 0b11,
    } << 2;
    // The order buy token balance is encoded as 1 bit in position 4.
    result |= match order.buy_token_balance {
        order::BuyTokenBalance::Erc20 => 0b0,
        order::BuyTokenBalance::Internal => 0b1,
    } << 4;
    // The signing scheme is encoded as a 2 bits in position 5.
    result |= match order.signature.scheme {
        order::signature::Scheme::Eip712 => 0b00,
        order::signature::Scheme::EthSign => 0b01,
        order::signature::Scheme::Eip1271 => 0b10,
        order::signature::Scheme::PreSign => 0b11,
    } << 5;
    result.into()
}

fn approve(allowance: &Allowance) -> eth::Interaction {
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

struct Trade {
    sell_token_index: eth::U256,
    buy_token_index: eth::U256,
    receiver: eth::H160,
    sell_amount: eth::U256,
    buy_amount: eth::U256,
    valid_to: u32,
    app_data: Bytes<[u8; 32]>,
    fee_amount: eth::U256,
    flags: eth::U256,
    executed_amount: eth::U256,
    signature: Bytes<Vec<u8>>,
}

mod codec {
    use crate::domain::eth;

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

    pub fn trade(trade: &super::Trade) -> Trade {
        (
            trade.sell_token_index,
            trade.buy_token_index,
            trade.receiver,
            trade.sell_amount,
            trade.buy_amount,
            trade.valid_to,
            ethcontract::Bytes(trade.app_data.into()),
            trade.fee_amount,
            trade.flags,
            trade.executed_amount,
            ethcontract::Bytes(trade.signature.0.clone()),
        )
    }

    type Interaction = (
        eth::H160,                   // target
        eth::U256,                   // value
        ethcontract::Bytes<Vec<u8>>, // signature
    );

    pub fn interaction(interaction: &eth::Interaction) -> Interaction {
        (
            interaction.target.0,
            interaction.value.0,
            ethcontract::Bytes(interaction.call_data.0.clone()),
        )
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

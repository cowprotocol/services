//! This module contains the logic for decoding the function input for
//! GPv2Settlement::settle function.

use {
    anyhow::{Context, Result},
    bigdecimal::{Signed, Zero},
    contracts::GPv2Settlement,
    ethcontract::{common::FunctionExt, tokens::Tokenize, Address, Bytes, H160, U256},
    model::{order::OrderKind, signature::Signature},
    num::BigRational,
    number_conversions::{big_decimal_to_u256, big_rational_to_u256, u256_to_big_rational},
    shared::{
        conversions::U256Ext,
        db_order_conversions::{order_kind_from, signing_scheme_from},
        external_prices::ExternalPrices,
    },
    web3::ethabi::{Function, Token},
};

// Original type for input of `GPv2Settlement.settle` function.
type DecodedSettlementTokenized = (
    Vec<Address>,
    Vec<U256>,
    Vec<(
        U256,            // sellTokenIndex
        U256,            // buyTokenIndex
        Address,         // receiver
        U256,            // sellAmount
        U256,            // buyAmount
        u32,             // validTo
        Bytes<[u8; 32]>, // appData
        U256,            // feeAmount
        U256,            // flags
        U256,            // executedAmount
        Bytes<Vec<u8>>,  // signature
    )>,
    [Vec<(Address, U256, Bytes<Vec<u8>>)>; 3],
);

#[derive(Debug)]
pub struct DecodedSettlement {
    // TODO check if `EncodedSettlement` can be reused
    pub tokens: Vec<Address>,
    pub clearing_prices: Vec<U256>,
    pub trades: Vec<DecodedTrade>,
    pub interactions: [Vec<DecodedInteraction>; 3],
}

#[derive(Debug)]
pub struct DecodedTrade {
    pub sell_token_index: U256,
    pub buy_token_index: U256,
    pub receiver: Address,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub valid_to: u32,
    pub app_data: Bytes<[u8; 32]>,
    pub fee_amount: U256,
    pub flags: TradeFlags,
    pub executed_amount: U256,
    pub signature: Bytes<Vec<u8>>,
}

impl DecodedTrade {
    fn matches_execution(&self, order: &OrderExecution) -> bool {
        let matches_order = self.signature.0 == order.signature;

        // the `executed_amount` field is ignored by the smart contract for
        // fill-or-kill orders, so only check that executed amounts match for
        // partially fillable orders.
        let matches_execution =
            !self.flags.partially_fillable() || self.executed_amount == order.executed_amount;

        matches_order && matches_execution
    }
}

/// Trade flags are encoded in a 256-bit integer field. For more information on
/// how flags are encoded see:
/// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/libraries/GPv2Trade.sol#L58-L94>
#[derive(Debug)]
pub struct TradeFlags(pub U256);

impl TradeFlags {
    fn as_u8(&self) -> u8 {
        self.0.byte(0)
    }

    fn order_kind(&self) -> OrderKind {
        if self.as_u8() & 0b1 == 0 {
            OrderKind::Sell
        } else {
            OrderKind::Buy
        }
    }

    fn partially_fillable(&self) -> bool {
        self.as_u8() & 0b10 != 0
    }
}

impl From<U256> for TradeFlags {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub struct DecodedInteraction {
    pub target: Address,
    pub value: U256,
    pub call_data: Bytes<Vec<u8>>,
}

impl From<(Address, U256, Bytes<Vec<u8>>)> for DecodedInteraction {
    fn from((target, value, call_data): (Address, U256, Bytes<Vec<u8>>)) -> Self {
        Self {
            target,
            value,
            call_data,
        }
    }
}

impl From<DecodedSettlementTokenized> for DecodedSettlement {
    fn from((tokens, clearing_prices, trades, interactions): DecodedSettlementTokenized) -> Self {
        DecodedSettlement {
            tokens,
            clearing_prices,
            trades: trades
                .into_iter()
                .map(|trade| DecodedTrade {
                    sell_token_index: trade.0,
                    buy_token_index: trade.1,
                    receiver: trade.2,
                    sell_amount: trade.3,
                    buy_amount: trade.4,
                    valid_to: trade.5,
                    app_data: trade.6,
                    fee_amount: trade.7,
                    flags: trade.8.into(),
                    executed_amount: trade.9,
                    signature: trade.10,
                })
                .collect(),
            interactions: interactions.map(|inner| inner.into_iter().map(Into::into).collect()),
        }
    }
}

/// It's possible that the same order gets filled in portions across multiple or
/// even the same settlement. This struct describes the details of such a fill.
/// Note that most orders only have a single fill as they are fill-or-kill
/// orders but partially fillable orders could have associated any number of
/// [`OrderExecution`]s with them.
#[derive(Debug, Clone)]
pub struct OrderExecution {
    pub executed_solver_fee: Option<U256>,
    pub kind: OrderKind,
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub executed_amount: U256,
    pub signature: Vec<u8>, //encoded signature
}

impl TryFrom<database::orders::OrderExecution> for OrderExecution {
    type Error = anyhow::Error;

    fn try_from(order: database::orders::OrderExecution) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            executed_solver_fee: order
                .executed_solver_fee
                .as_ref()
                .and_then(big_decimal_to_u256),
            kind: order_kind_from(order.kind),
            sell_token: H160(order.sell_token.0),
            buy_token: H160(order.buy_token.0),
            sell_amount: big_decimal_to_u256(&order.sell_amount).context("sell_amount")?,
            buy_amount: big_decimal_to_u256(&order.buy_amount).context("buy_amount")?,
            executed_amount: big_decimal_to_u256(&order.executed_amount).unwrap(),
            signature: {
                let signing_scheme = signing_scheme_from(order.signing_scheme);
                let signature = Signature::from_bytes(signing_scheme, &order.signature)?;
                signature
                    .encode_for_settlement(H160(order.owner.0))
                    .to_vec()
            },
        })
    }
}

impl DecodedSettlement {
    pub fn new(input: &[u8]) -> Result<Self, DecodingError> {
        let function = GPv2Settlement::raw_contract()
            .abi
            .function("settle")
            .unwrap();
        let decoded_input = decode_function_input(function, input)?;
        <DecodedSettlementTokenized>::from_token(Token::Tuple(decoded_input))
            .map(Into::into)
            .context("failed to decode settlement")
            .map_err(From::from)
    }

    /// Returns the total surplus denominated in the native asset for the
    /// solution.
    pub fn total_surplus(&self, external_prices: &ExternalPrices) -> U256 {
        self.trades.iter().fold(0.into(), |acc, trade| {
            acc + match surplus(trade, &self.tokens, &self.clearing_prices, external_prices) {
                Some(surplus) => surplus,
                None => {
                    tracing::warn!("possible incomplete surplus calculation");
                    0.into()
                }
            }
        })
    }

    /// Returns the total `executed_solver_fee` of this solution converted to
    /// the native token. This is only the value used for objective value
    /// computatations and can theoretically be different from the value of
    /// fees actually collected by the protocol.
    pub fn total_fees(
        &self,
        external_prices: &ExternalPrices,
        mut orders: Vec<OrderExecution>,
    ) -> U256 {
        self.trades.iter().fold(0.into(), |acc, trade| {
            match orders
                .iter()
                .position(|order| trade.matches_execution(order))
            {
                Some(i) => {
                    // It's possible to have multiple fills with the same `executed_amount` for the
                    // same order with different `solver_fees`. To end up with the correct total
                    // fees we can only use every `OrderExecution` exactly once.
                    let order = orders.swap_remove(i);
                    acc + match self.fee(external_prices, &order, trade) {
                        Some(fee) => fee,
                        None => {
                            tracing::warn!("possible incomplete fee calculation");
                            0.into()
                        }
                    }
                }
                None => {
                    tracing::warn!(
                        "order not found for trade, possible incomplete fee calculation"
                    );
                    acc
                }
            }
        })
    }

    fn fee(
        &self,
        external_prices: &ExternalPrices,
        order: &OrderExecution,
        trade: &DecodedTrade,
    ) -> Option<U256> {
        let solver_fee = match &order.executed_solver_fee {
            Some(solver_fee) => u256_to_big_rational(solver_fee),
            None => {
                // this should be a partial limit order
                if !trade.flags.partially_fillable() {
                    tracing::warn!("missing solver fee for non partial fee order");
                    return None;
                }

                // get uniform prices
                let sell_index = self
                    .tokens
                    .iter()
                    .position(|token| token == &order.sell_token)?;
                let buy_index = self
                    .tokens
                    .iter()
                    .position(|token| token == &order.buy_token)?;
                let uniform_sell_price = self.clearing_prices.get(sell_index).cloned()?;
                let uniform_buy_price = self.clearing_prices.get(buy_index).cloned()?;

                // get executed(adjusted) prices
                let sell_index = trade.sell_token_index.as_u64() as usize;
                let buy_index = trade.buy_token_index.as_u64() as usize;
                let adjusted_sell_price = self.clearing_prices.get(sell_index)?;
                let adjusted_buy_price = self.clearing_prices.get(buy_index)?;

                // the logic is opposite to the code in function `custom_price_for_limit_order`
                let fee = match trade.flags.order_kind() {
                    OrderKind::Buy => {
                        let executed_sell_amount = trade
                            .executed_amount
                            .checked_mul(uniform_buy_price)?
                            .checked_div(uniform_sell_price)?;
                        adjusted_buy_price.checked_sub(executed_sell_amount)?
                    }
                    OrderKind::Sell => {
                        let sell_amount = adjusted_sell_price
                            .checked_mul(uniform_buy_price)?
                            .checked_div(uniform_sell_price)?;
                        sell_amount.checked_sub(trade.executed_amount)?
                    }
                };

                u256_to_big_rational(&fee)
            }
        };

        // converts the order's `solver_fee` which is denominated in `sell_token` to the
        // native token.
        tracing::trace!(?solver_fee, "fee before conversion to native token");
        let fee = external_prices.try_get_native_amount(order.sell_token, solver_fee)?;
        tracing::trace!(?fee, "fee after conversion to native token");
        big_rational_to_u256(&fee).ok()
    }
}

fn surplus(
    trade: &DecodedTrade,
    tokens: &[Address],
    clearing_prices: &[U256],
    external_prices: &ExternalPrices,
) -> Option<U256> {
    let sell_token_index = trade.sell_token_index.as_u64() as usize;
    let buy_token_index = trade.buy_token_index.as_u64() as usize;

    let sell_token_clearing_price = clearing_prices.get(sell_token_index)?.to_big_rational();
    let buy_token_clearing_price = clearing_prices.get(buy_token_index)?.to_big_rational();
    let kind = trade.flags.order_kind();

    if match kind {
        OrderKind::Sell => &buy_token_clearing_price,
        OrderKind::Buy => &sell_token_clearing_price,
    }
    .is_zero()
    {
        return None;
    }

    let surplus = trade_surplus(
        kind,
        &trade.sell_amount.to_big_rational(),
        &trade.buy_amount.to_big_rational(),
        &trade.executed_amount.to_big_rational(),
        &sell_token_clearing_price,
        &buy_token_clearing_price,
    )?;

    let normalized_surplus = match kind {
        OrderKind::Sell => {
            let buy_token = tokens.get(buy_token_index)?;
            external_prices.try_get_native_amount(*buy_token, surplus / buy_token_clearing_price)?
        }
        OrderKind::Buy => {
            let sell_token = tokens.get(sell_token_index)?;
            external_prices
                .try_get_native_amount(*sell_token, surplus / sell_token_clearing_price)?
        }
    };

    big_rational_to_u256(&normalized_surplus).ok()
}

fn trade_surplus(
    kind: OrderKind,
    sell_amount: &BigRational,
    buy_amount: &BigRational,
    executed_amount: &BigRational,
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
) -> Option<BigRational> {
    match kind {
        OrderKind::Buy => buy_order_surplus(
            sell_token_price,
            buy_token_price,
            sell_amount,
            buy_amount,
            executed_amount,
        ),
        OrderKind::Sell => sell_order_surplus(
            sell_token_price,
            buy_token_price,
            sell_amount,
            buy_amount,
            executed_amount,
        ),
    }
}

// The difference between what you were willing to sell (executed_amount *
// limit_price) converted into reference token (multiplied by buy_token_price)
// and what you had to sell denominated in the reference token (executed_amount
// * buy_token_price)
fn buy_order_surplus(
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
    sell_amount_limit: &BigRational,
    buy_amount_limit: &BigRational,
    executed_buy_amount: &BigRational,
) -> Option<BigRational> {
    if buy_amount_limit.is_zero() {
        return None;
    }
    let limit_sell_amount = executed_buy_amount * sell_amount_limit / buy_amount_limit;
    let res = (limit_sell_amount * sell_token_price) - (executed_buy_amount * buy_token_price);
    if res.is_negative() {
        None
    } else {
        Some(res)
    }
}

// The difference of your proceeds denominated in the reference token
// (executed_sell_amount * sell_token_price) and what you were minimally willing
// to receive in buy tokens (executed_sell_amount * limit_price) converted to
// amount in reference token at the effective price (multiplied by
// buy_token_price)
fn sell_order_surplus(
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
    sell_amount_limit: &BigRational,
    buy_amount_limit: &BigRational,
    executed_sell_amount: &BigRational,
) -> Option<BigRational> {
    if sell_amount_limit.is_zero() {
        return None;
    }
    let limit_buy_amount = executed_sell_amount * buy_amount_limit / sell_amount_limit;
    let res = (executed_sell_amount * sell_token_price) - (limit_buy_amount * buy_token_price);
    if res.is_negative() {
        None
    } else {
        Some(res)
    }
}

#[derive(Debug)]
pub enum DecodingError {
    InvalidSelector,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for DecodingError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<DecodingError> for anyhow::Error {
    fn from(err: DecodingError) -> Self {
        match err {
            DecodingError::InvalidSelector => anyhow::anyhow!("invalid function selector"),
            DecodingError::Other(err) => err,
        }
    }
}

/// `input` is the raw call data from the transaction receipt.
/// Example: `13d79a0b00000000` where `13d79a0b` is the function selector for
/// `settle` function in case of GPv2Settlement contract.
pub fn decode_function_input(
    function: &Function,
    input: &[u8],
) -> Result<Vec<Token>, DecodingError> {
    let input = input
        .strip_prefix(&function.selector())
        .ok_or(DecodingError::InvalidSelector)?;
    let decoded_input = function
        .decode_input(input)
        .context("decode input failed")?;
    Ok(decoded_input)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ethcontract::H160,
        shared::ethrpc::Web3,
        std::{collections::BTreeMap, str::FromStr},
    };

    #[tokio::test]
    #[ignore]
    async fn total_surplus_test() {
        // transaction hash:
        // 0x4ed25533ae840fa36951c670b1535265977491b8c4db38d6fe3b2cffe3dad298

        // From solver competition table:

        // external prices (auction values):
        // 0x0f2d719407fdbeff09d87557abb7232601fd9f29: 773763471505852
        // 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48: 596635491559324261891964928
        // 0xdac17f958d2ee523a2206206994597c13d831ec7: 596703190526849003475173376
        // 0xf4d2888d29d722226fafa5d9b24f9164c092421e: 130282568907757

        // surplus: 33350701806766732

        let transport = shared::ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);
        let native_token = contracts::WETH9::deployed(&web3).await.unwrap().address();
        let call_data = hex_literal::hex!(
            "13d79a0b0000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000005e
            000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000f2d719407fdbeff09d87557abb7232601fd9f29000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec70000000
            00000000000000000f4d2888d29d722226fafa5d9b24f9164c092421e00000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000dd3fd65500000000000000000000000000000000000000000000009b1d8dff36ae3000000000000000000000
            0000000000000000000000000000009a8038306f85f00000000000000000000000000000000000000000000000000000000000002540be4000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000
            0000000000000000000000000000000000000022000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e995e2a9ae5210feb6dd07618af28ec38b2d7ce10000000000000000000000000000000
            00000000000000000000000037b64751300000000000000000000000000000000000000000000026c80b0ff052d91ac660000000000000000000000000000000000000000000000000000000063f4d8c4c86d3a0def4d16bd04317645da9ae1d6871726d8adf83a0695447f8ee5c63d12000000000000000000000000000000000000000
            0000000000000000002ad60ed0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000037b647513000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000
            00000000000000041155ff208365bbf30585f5b18fc92d766e46121a1963f903bb6f3f77e5d0eaefb27abc4831ce1f837fcb70e11d4e4d97474c677469240849d69e17f7173aead841b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
            0000000030000000000000000000000000000000000000000000000000000000000000001000000000000000000000000f352bffb3e902d78166a79c9878e138a65022e1100000000000000000000000000000000000000000000013519ef49947442f04d0000000000000000000000000000000000000000000000000000000049b4e9b
            80000000000000000000000000000000000000000000000000000000063f4d8bbc86d3a0def4d16bd04317645da9ae1d6871726d8adf83a0695447f8ee5c63d1200000000000000000000000000000000000000000000000575a7d4f1093bc00000000000000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000013519ef49947442f04d00000000000000000000000000000000000000000000000000000000000001600000000000000000000000000000000000000000000000000000000000000041882a1c875ff1316bb79bde0d0792869f784d58097d8489a722519e6417c577cf5cc745a2e353298
            dea6514036d5eb95563f8f7640e20ef0fd41b10ccbdfc87641b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000008000000000000000000000000
            00000000000000000000000000000000000000a800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000900000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000
            00000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002e000000000000000000000000000000000000000000000000000000000000003e000000000000000000000000000000000000000000000000000000000000004e0000000000000000000000000000000000000000
            00000000000000000000005c00000000000000000000000000000000000000000000000000000000000000720000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000008e0000000000000000000000000ce0beb5db55754c14cdfa13
            3ec2268d4486f965600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000004401c6adc3000000000000000000000000a0b86991c6218b36c1d19d4
            a2e9eb0ce3606eb48000000000000000000000000000000000000000000000000000000004a3c099600000000000000000000000000000000000000000000000000000000000000000000000000000000ce0beb5db55754c14cdfa133ec2268d4486f9656000000000000000000000000000000000000000000000000000000000000000
            00000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000004401c6adc3000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000405ff0dca143cb5
            2000000000000000000000000000000000000000000000000000000000000000000000000000000001d94bedcb3641ba060091ed090d28bbdccdb7f1d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000
            000000000000000000000000000000000000000000000006420cf38cc00000000000000000000000000000000000000000000000000000001abde4cad00000000000000000000000000000000000000000000000000000001aaaee8008000000000000000000000003416cf6c708da44db2624d63ea0aaef7113527c6000000000000000
            000000000000000000000000000000000000000000000000000000000000000001d94bedcb3641ba060091ed090d28bbdccdb7f1d000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000
            00000000000000000000000000000006420cf38cc00000000000000000000000000000000000000000000013519ef49947442f04d0000000000000000000000000000000000000000000000000a34eb03000000008000000000000000000000004b5ab61593a2401b1075b90c04cbcdd3f87ce0110000000000000000000000000000000
            0000000000000000000000000000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb480000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000
            00000000000000044a9059cbb00000000000000000000000005104ebba2b6d3b8254aa41cf6df80462f6160ae00000000000000000000000000000000000000000000000000000001abe1cd590000000000000000000000000000000000000000000000000000000000000000000000000000000005104ebba2b6d3b8254aa41cf6df804
            62f6160ae0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000c4022c0d9f00000000000000000000000000000000000000000000012b1445dfc
            eb244cadb00000000000000000000000000000000000000000000000000000000000000000000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab410000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000000000000000000000
            00000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044a9059cbb00000000000000000000000005e3734ff2b3127e01070eb225afe910525959ad0000000000000000000000000000000000000000000000000a4f4fa622eb5980000000000000000
            00000000000000000000000000000000000000000000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000
            000000000000000000000000000000044a9059cbb00000000000000000000000005e3734ff2b3127e01070eb225afe910525959ad00000000000000000000000000000000000000000000000000000001cf862866000000000000000000000000000000000000000000000000000000000000000000000000000000001d94bedcb3641ba
            060091ed090d28bbdccdb7f1d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000006420cf38cc000000000000000000000000000000000000000
            000000000405ff0dca143cb520000000000000000000000000000000000000000000001428c970000000000008000000000000000000000002dd35b4da6534230ff53048f7477f17f7f4e7a70000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
            000000000"
        );
        let settlement = DecodedSettlement::new(&call_data).unwrap();

        //calculate surplus
        let auction_external_prices = BTreeMap::from([
            (
                H160::from_str("0x0f2d719407fdbeff09d87557abb7232601fd9f29").unwrap(),
                U256::from(773763471505852u128),
            ),
            (
                H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                U256::from(596635491559324261891964928u128),
            ),
            (
                H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap(),
                U256::from(596703190526849003475173376u128),
            ),
            (
                H160::from_str("0xf4d2888d29d722226fafa5d9b24f9164c092421e").unwrap(),
                U256::from(130282568907757u128),
            ),
        ]);
        let external_prices =
            ExternalPrices::try_from_auction_prices(native_token, auction_external_prices).unwrap();
        let surplus = settlement.total_surplus(&external_prices).to_f64_lossy(); // to_f64_lossy() to mimic what happens when value is saved for solver
                                                                                 // competition
        assert_eq!(surplus, 33350701806766732.);
    }

    #[tokio::test]
    #[ignore]
    async fn total_fees_test() {
        // transaction hash:
        // 0x4ed25533ae840fa36951c670b1535265977491b8c4db38d6fe3b2cffe3dad298

        // From solver competition table:

        // external prices (auction values):
        // 0x0f2d719407fdbeff09d87557abb7232601fd9f29: 773763471505852
        // 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48: 596635491559324261891964928
        // 0xdac17f958d2ee523a2206206994597c13d831ec7: 596703190526849003475173376
        // 0xf4d2888d29d722226fafa5d9b24f9164c092421e: 130282568907757

        // fees: 45377573614605000

        let transport = shared::ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);
        let native_token = contracts::WETH9::deployed(&web3).await.unwrap().address();
        let call_data = hex_literal::hex!(
            "13d79a0b0000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000005e
            000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000f2d719407fdbeff09d87557abb7232601fd9f29000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec70000000
            00000000000000000f4d2888d29d722226fafa5d9b24f9164c092421e00000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000dd3fd65500000000000000000000000000000000000000000000009b1d8dff36ae3000000000000000000000
            0000000000000000000000000000009a8038306f85f00000000000000000000000000000000000000000000000000000000000002540be4000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000
            0000000000000000000000000000000000000022000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e995e2a9ae5210feb6dd07618af28ec38b2d7ce10000000000000000000000000000000
            00000000000000000000000037b64751300000000000000000000000000000000000000000000026c80b0ff052d91ac660000000000000000000000000000000000000000000000000000000063f4d8c4c86d3a0def4d16bd04317645da9ae1d6871726d8adf83a0695447f8ee5c63d12000000000000000000000000000000000000000
            0000000000000000002ad60ed0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000037b647513000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000
            00000000000000041155ff208365bbf30585f5b18fc92d766e46121a1963f903bb6f3f77e5d0eaefb27abc4831ce1f837fcb70e11d4e4d97474c677469240849d69e17f7173aead841b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
            0000000030000000000000000000000000000000000000000000000000000000000000001000000000000000000000000f352bffb3e902d78166a79c9878e138a65022e1100000000000000000000000000000000000000000000013519ef49947442f04d0000000000000000000000000000000000000000000000000000000049b4e9b
            80000000000000000000000000000000000000000000000000000000063f4d8bbc86d3a0def4d16bd04317645da9ae1d6871726d8adf83a0695447f8ee5c63d1200000000000000000000000000000000000000000000000575a7d4f1093bc00000000000000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000013519ef49947442f04d00000000000000000000000000000000000000000000000000000000000001600000000000000000000000000000000000000000000000000000000000000041882a1c875ff1316bb79bde0d0792869f784d58097d8489a722519e6417c577cf5cc745a2e353298
            dea6514036d5eb95563f8f7640e20ef0fd41b10ccbdfc87641b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000008000000000000000000000000
            00000000000000000000000000000000000000a800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000900000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000
            00000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002e000000000000000000000000000000000000000000000000000000000000003e000000000000000000000000000000000000000000000000000000000000004e0000000000000000000000000000000000000000
            00000000000000000000005c00000000000000000000000000000000000000000000000000000000000000720000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000008e0000000000000000000000000ce0beb5db55754c14cdfa13
            3ec2268d4486f965600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000004401c6adc3000000000000000000000000a0b86991c6218b36c1d19d4
            a2e9eb0ce3606eb48000000000000000000000000000000000000000000000000000000004a3c099600000000000000000000000000000000000000000000000000000000000000000000000000000000ce0beb5db55754c14cdfa133ec2268d4486f9656000000000000000000000000000000000000000000000000000000000000000
            00000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000004401c6adc3000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000405ff0dca143cb5
            2000000000000000000000000000000000000000000000000000000000000000000000000000000001d94bedcb3641ba060091ed090d28bbdccdb7f1d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000
            000000000000000000000000000000000000000000000006420cf38cc00000000000000000000000000000000000000000000000000000001abde4cad00000000000000000000000000000000000000000000000000000001aaaee8008000000000000000000000003416cf6c708da44db2624d63ea0aaef7113527c6000000000000000
            000000000000000000000000000000000000000000000000000000000000000001d94bedcb3641ba060091ed090d28bbdccdb7f1d000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000
            00000000000000000000000000000006420cf38cc00000000000000000000000000000000000000000000013519ef49947442f04d0000000000000000000000000000000000000000000000000a34eb03000000008000000000000000000000004b5ab61593a2401b1075b90c04cbcdd3f87ce0110000000000000000000000000000000
            0000000000000000000000000000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb480000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000
            00000000000000044a9059cbb00000000000000000000000005104ebba2b6d3b8254aa41cf6df80462f6160ae00000000000000000000000000000000000000000000000000000001abe1cd590000000000000000000000000000000000000000000000000000000000000000000000000000000005104ebba2b6d3b8254aa41cf6df804
            62f6160ae0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000c4022c0d9f00000000000000000000000000000000000000000000012b1445dfc
            eb244cadb00000000000000000000000000000000000000000000000000000000000000000000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab410000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000000000000000000000
            00000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044a9059cbb00000000000000000000000005e3734ff2b3127e01070eb225afe910525959ad0000000000000000000000000000000000000000000000000a4f4fa622eb5980000000000000000
            00000000000000000000000000000000000000000000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000
            000000000000000000000000000000044a9059cbb00000000000000000000000005e3734ff2b3127e01070eb225afe910525959ad00000000000000000000000000000000000000000000000000000001cf862866000000000000000000000000000000000000000000000000000000000000000000000000000000001d94bedcb3641ba
            060091ed090d28bbdccdb7f1d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000006420cf38cc000000000000000000000000000000000000000
            000000000405ff0dca143cb520000000000000000000000000000000000000000000001428c970000000000008000000000000000000000002dd35b4da6534230ff53048f7477f17f7f4e7a70000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
            000000000"
        );
        let settlement = DecodedSettlement::new(&call_data).unwrap();

        //calculate fees
        let auction_external_prices = BTreeMap::from([
            (
                H160::from_str("0x0f2d719407fdbeff09d87557abb7232601fd9f29").unwrap(),
                U256::from(773763471505852u128),
            ),
            (
                H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                U256::from(596635491559324261891964928u128),
            ),
            (
                H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap(),
                U256::from(596703190526849003475173376u128),
            ),
            (
                H160::from_str("0xf4d2888d29d722226fafa5d9b24f9164c092421e").unwrap(),
                U256::from(130282568907757u128),
            ),
        ]);
        let external_prices =
            ExternalPrices::try_from_auction_prices(native_token, auction_external_prices).unwrap();

        let orders = vec![
            OrderExecution {
                executed_solver_fee: Some(48263037u128.into()),
                kind: OrderKind::Sell,
                buy_amount: 11446254517730382294118u128.into(),
                sell_amount: 14955083027u128.into(),
                sell_token: H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap(),
                buy_token: Default::default(),
                executed_amount: 14955083027u128.into(),
                signature: hex::decode("155ff208365bbf30585f5b18fc92d766e46121a1963f903bb6f3f77e5d0eaefb27abc4831ce1f837fcb70e11d4e4d97474c677469240849d69e17f7173aead841b").unwrap(),
            },
            OrderExecution {
                executed_solver_fee: Some(127253135942751092736u128.into()),
                kind: OrderKind::Sell,
                buy_amount: 1236593080.into(),
                sell_amount: 5701912712048588025933u128.into(),
                sell_token: H160::from_str("0xf4d2888d29d722226fafa5d9b24f9164c092421e").unwrap(),
                buy_token: Default::default(),
                executed_amount: 5701912712048588025933u128.into(),
                signature: hex::decode("882a1c875ff1316bb79bde0d0792869f784d58097d8489a722519e6417c577cf5cc745a2e353298dea6514036d5eb95563f8f7640e20ef0fd41b10ccbdfc87641b").unwrap(),
            }
        ];
        let fees = settlement
            .total_fees(&external_prices, orders)
            .to_f64_lossy(); // to_f64_lossy() to mimic what happens when value is saved for solver
                             // competition
        assert_eq!(fees, 45377573614605000.);
    }

    #[tokio::test]
    #[ignore]
    async fn execution_amount_does_not_matter_for_fok_orders() {
        // transaction hash:
        // 0x

        // From solver competition table:

        // external prices (auction values):
        // 0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee: 1000000000000000000
        // 0xf88baf18fab7e330fa0c4f83949e23f52fececce: 29428019732094

        // fees: 463182886014406361088

        let transport = shared::ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);
        let native_token = contracts::WETH9::deployed(&web3).await.unwrap().address();
        let call_data = hex_literal::hex!(
            "13d79a0b
             0000000000000000000000000000000000000000000000000000000000000080
             00000000000000000000000000000000000000000000000000000000000000e0
             0000000000000000000000000000000000000000000000000000000000000140
             0000000000000000000000000000000000000000000000000000000000000360
             0000000000000000000000000000000000000000000000000000000000000002
             000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
             000000000000000000000000f88baf18fab7e330fa0c4f83949e23f52fececce
             0000000000000000000000000000000000000000000000000000000000000002
             000000000000000000000000000000000000000000000000000132e67578cc3f
             00000000000000000000000000000000000000000000000000000002540be400
             0000000000000000000000000000000000000000000000000000000000000001
             0000000000000000000000000000000000000000000000000000000000000020
             0000000000000000000000000000000000000000000000000000000000000001
             0000000000000000000000000000000000000000000000000000000000000000
             000000000000000000000000b70cd1ebd3b24aeeaf90c6041446630338536e7f
             0000000000000000000000000000000000000000000000a41648a28d9cdecee6
             000000000000000000000000000000000000000000000000013d0a4d504284e9
             00000000000000000000000000000000000000000000000000000000643d6a39
             e9f29ae547955463ed535162aefee525d8d309571a2b18bc26086c8c35d781eb
             00000000000000000000000000000000000000000000002557f7974fde5c0000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000000000a41648a28d9cdecee6
             0000000000000000000000000000000000000000000000000000000000000160
             0000000000000000000000000000000000000000000000000000000000000041
             4935ea3f24155f6757df94d8c0bc96665d46da51e1a8e39d935967c9216a6091
             2fa50a5393a323d453c78d179d0199ddd58f6d787781e4584357d3e0205a7600
             1c00000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             0000000000000000000000000000000000000000000000000000000000000080
             0000000000000000000000000000000000000000000000000000000000000420
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000002
             0000000000000000000000000000000000000000000000000000000000000040
             00000000000000000000000000000000000000000000000000000000000002c0
             000000000000000000000000ba12222222228d8ba445958a75a0704d566bf2c8
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             00000000000000000000000000000000000000000000000000000000000001e4
             52bbbe2900000000000000000000000000000000000000000000000000000000
             000000e00000000000000000000000009008d19f58aabd9ed0d60971565aa851
             0560ab4100000000000000000000000000000000000000000000000000000000
             000000000000000000000000000000009008d19f58aabd9ed0d60971565aa851
             0560ab4100000000000000000000000000000000000000000000000000000000
             000000000000000000000000000000000000000000000000000000a566558000
             0000000000000000000000000000000000000000000000000000000000000001
             0000000067f117350eab45983374f4f83d275d8a5d62b1bf0001000000000000
             000004f200000000000000000000000000000000000000000000000000000000
             00000001000000000000000000000000f88baf18fab7e330fa0c4f83949e23f5
             2fececce000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead908
             3c756cc2000000000000000000000000000000000000000000000000013eae86
             d49c295900000000000000000000000000000000000000000000000000000000
             000000c000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000060
             0000000000000000000000000000000000000000000000000000000000000024
             2e1a7d4d000000000000000000000000000000000000000000000000013eae86
             d49c29bf00000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000"
        );
        let settlement = DecodedSettlement::new(&call_data).unwrap();

        //calculate fees
        let auction_external_prices = BTreeMap::from([
            (
                H160::from_str("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap(),
                U256::from(1000000000000000000u128),
            ),
            (
                H160::from_str("0xf88baf18fab7e330fa0c4f83949e23f52fececce").unwrap(),
                U256::from(29428019732094u128),
            ),
        ]);
        let external_prices =
            ExternalPrices::try_from_auction_prices(native_token, auction_external_prices).unwrap();

        let orders = vec![
            OrderExecution {
                executed_solver_fee: Some(463182886014406361088u128.into()),
                kind: OrderKind::Sell,
                buy_amount: 89238894792574185u128.into(),
                sell_amount: 3026871740084629982950u128.into(),
                sell_token: H160::from_str("0xf88baf18fab7e330fa0c4f83949e23f52fececce").unwrap(),
                buy_token: H160::from_str("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap(),
                executed_amount: 0.into(),
                signature: hex::decode("4935ea3f24155f6757df94d8c0bc96665d46da51e1a8e39d935967c9216a60912fa50a5393a323d453c78d179d0199ddd58f6d787781e4584357d3e0205a76001c").unwrap(),
            },
        ];
        let fees = settlement
            .total_fees(&external_prices, orders)
            .to_f64_lossy();
        assert_eq!(fees, 13630555109200196.);
    }
}

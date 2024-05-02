//! This module contains the logic for decoding the function input for
//! GPv2Settlement::settle function.

use {
    anyhow::{Context, Result},
    app_data::AppDataHash,
    bigdecimal::{Signed, Zero},
    contracts::GPv2Settlement,
    ethcontract::{common::FunctionExt, tokens::Tokenize, Address, Bytes, U256},
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, OrderUid, SellTokenSource},
        signature::{Signature, SigningScheme},
        DomainSeparator,
    },
    num::BigRational,
    number::conversions::{big_rational_to_u256, u256_to_big_rational},
    shared::{conversions::U256Ext, external_prices::ExternalPrices},
    std::collections::HashSet,
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

#[derive(Debug, PartialEq, Eq)]
pub struct DecodedSettlement {
    // TODO check if `EncodedSettlement` can be reused
    pub tokens: Vec<Address>,
    pub clearing_prices: Vec<U256>,
    pub trades: Vec<TradeWithOrderUid>,
    pub interactions: [Vec<DecodedInteraction>; 3],
    /// Data that was appended to the regular call data of the `settle()` call
    /// as a form of on-chain meta data. This gets used to associated a
    /// settlement with an auction.
    pub metadata: Option<Bytes<[u8; Self::META_DATA_LEN]>>,
}

#[derive(Debug, PartialEq, Eq)]
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
    /// Returns the signature of the order.
    fn signature(&self) -> Result<Signature> {
        Signature::from_bytes(self.flags.signing_scheme(), &self.signature.0)
    }

    /// Returns the order uid of the order associated with this trade.
    pub fn uid(&self, domain_separator: &DomainSeparator, tokens: &[Address]) -> Result<OrderUid> {
        let order = OrderData {
            sell_token: tokens[self.sell_token_index.as_u64() as usize],
            buy_token: tokens[self.buy_token_index.as_u64() as usize],
            sell_amount: self.sell_amount,
            buy_amount: self.buy_amount,
            valid_to: self.valid_to,
            app_data: AppDataHash(self.app_data.0),
            fee_amount: self.fee_amount,
            kind: self.flags.order_kind(),
            partially_fillable: self.flags.partially_fillable(),
            receiver: Some(self.receiver),
            sell_token_balance: self.flags.sell_token_balance(),
            buy_token_balance: self.flags.buy_token_balance(),
        };
        let owner = self
            .signature()
            .context("signature is invalid")?
            .recover_owner(&self.signature.0, domain_separator, &order.hash_struct())
            .context("cant recover owner")?;
        Ok(order.uid(domain_separator, &owner))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TradeWithOrderUid {
    pub inner: DecodedTrade,
    pub order_uid: OrderUid,
}

/// Trade flags are encoded in a 256-bit integer field. For more information on
/// how flags are encoded see:
/// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/libraries/GPv2Trade.sol#L58-L94>
#[derive(Debug, PartialEq, Eq)]
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

    fn sell_token_balance(&self) -> SellTokenSource {
        if self.as_u8() & 0x08 == 0 {
            SellTokenSource::Erc20
        } else if self.as_u8() & 0x04 == 0 {
            SellTokenSource::External
        } else {
            SellTokenSource::Internal
        }
    }

    fn buy_token_balance(&self) -> BuyTokenDestination {
        if self.as_u8() & 0x10 == 0 {
            BuyTokenDestination::Erc20
        } else {
            BuyTokenDestination::Internal
        }
    }

    fn signing_scheme(&self) -> SigningScheme {
        match (self.as_u8() >> 5) & 0b11 {
            0b00 => SigningScheme::Eip712,
            0b01 => SigningScheme::EthSign,
            0b10 => SigningScheme::Eip1271,
            0b11 => SigningScheme::PreSign,
            _ => unreachable!(),
        }
    }
}

impl From<U256> for TradeFlags {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

#[derive(Debug, PartialEq, Eq)]
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

impl DecodedSettlement {
    /// Number of bytes that may be appended to the calldata to store an auction
    /// id.
    pub const META_DATA_LEN: usize = 8;

    pub fn new(input: &[u8], domain_separator: &DomainSeparator) -> Result<Self, DecodingError> {
        let function = GPv2Settlement::raw_contract()
            .interface
            .abi
            .function("settle")
            .unwrap();
        let without_selector = input
            .strip_prefix(&function.selector())
            .ok_or(DecodingError::InvalidSelector)?;

        // Decoding calldata without expecting metadata can succeed even if metadata
        // was appended. The other way around would not work so we do that first.
        if let Ok(decoded) = Self::try_new(without_selector, function, domain_separator, true) {
            return Ok(decoded);
        }
        Self::try_new(without_selector, function, domain_separator, false).map_err(Into::into)
    }

    fn try_new(
        data: &[u8],
        function: &Function,
        domain_separator: &DomainSeparator,
        with_metadata: bool,
    ) -> Result<Self> {
        let metadata_len = if with_metadata {
            anyhow::ensure!(
                data.len() % 32 == Self::META_DATA_LEN,
                "calldata does not contain the expected number of bytes to include metadata"
            );
            Self::META_DATA_LEN
        } else {
            0
        };

        let (calldata, metadata) = data.split_at(data.len() - metadata_len);
        let tokenized = function
            .decode_input(calldata)
            .context("tokenizing settlement calldata failed")?;
        let decoded = <DecodedSettlementTokenized>::from_token(Token::Tuple(tokenized))
            .context("decoding tokenized settlement calldata failed")?;

        let (tokens, clearing_prices, trades, interactions) = decoded;
        let trades = trades
            .into_iter()
            .filter_map(|trade| {
                let trade = DecodedTrade {
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
                };
                match trade.uid(domain_separator, &tokens) {
                    Ok(order_uid) => Some(TradeWithOrderUid {
                        inner: trade,
                        order_uid,
                    }),
                    Err(err) => {
                        tracing::error!(
                            ?err,
                            ?trade,
                            "failed to calculate order uid, we don't know which order this trade \
                             belongs to"
                        );
                        None
                    }
                }
            })
            .collect();
        Ok(Self {
            tokens,
            clearing_prices,
            trades,
            interactions: interactions.map(|inner| inner.into_iter().map(Into::into).collect()),
            metadata: metadata.try_into().ok().map(Bytes),
        })
    }

    /// Returns the total surplus denominated in the native asset for the
    /// solution.
    pub fn total_surplus(
        &self,
        external_prices: &ExternalPrices,
        jit_order_uids: HashSet<OrderUid>,
    ) -> U256 {
        self.trades
            .iter()
            .filter(|trade| !jit_order_uids.contains(&trade.order_uid))
            .fold(0.into(), |acc, trade| {
                acc + surplus(
                    &trade.inner,
                    &self.tokens,
                    &self.clearing_prices,
                    external_prices,
                )
                .unwrap_or_else(|| {
                    tracing::warn!("possible incomplete surplus calculation");
                    0.into()
                })
            })
    }

    /// Returns fees for all trades.
    pub fn all_fees(&self, external_prices: &ExternalPrices) -> Vec<Fees> {
        self.trades
            .iter()
            .map(|trade| {
                self.fee(trade, external_prices).unwrap_or_else(|| {
                    tracing::warn!("possible incomplete fee calculation");
                    // we should have an order execution for every trade
                    Fees {
                        order: trade.order_uid,
                        kind: FeeKind::User,
                        sell: U256::zero(),
                        native: U256::zero(),
                    }
                })
            })
            .collect()
    }

    fn fee(&self, trade: &TradeWithOrderUid, external_prices: &ExternalPrices) -> Option<Fees> {
        let sell_index = trade.inner.sell_token_index.as_u64() as usize;
        let buy_index = trade.inner.buy_token_index.as_u64() as usize;
        let sell_token = self.tokens.get(sell_index)?;
        let buy_token = self.tokens.get(buy_index)?;

        let (kind, fee) = match trade.inner.fee_amount.is_zero() {
            false => (FeeKind::User, trade.inner.fee_amount),
            true => {
                // get executed(adjusted) prices
                let adjusted_sell_price = self.clearing_prices.get(sell_index).cloned()?;
                let adjusted_buy_price = self.clearing_prices.get(buy_index).cloned()?;

                // get uniform prices
                let sell_index = self.tokens.iter().position(|token| token == sell_token)?;
                let buy_index = self.tokens.iter().position(|token| token == buy_token)?;
                let uniform_sell_price = self.clearing_prices.get(sell_index).cloned()?;
                let uniform_buy_price = self.clearing_prices.get(buy_index).cloned()?;

                // the logic is opposite to the code in function `custom_price_for_limit_order`
                let fee = match trade.inner.flags.order_kind() {
                    OrderKind::Buy => {
                        let required_sell_amount = trade
                            .inner
                            .executed_amount
                            .checked_mul(adjusted_buy_price)?
                            .checked_div(adjusted_sell_price)?;
                        let required_sell_amount_with_ucp = trade
                            .inner
                            .executed_amount
                            .checked_mul(uniform_buy_price)?
                            .checked_div(uniform_sell_price)?;
                        required_sell_amount.checked_sub(required_sell_amount_with_ucp)?
                    }
                    OrderKind::Sell => {
                        let received_buy_amount = trade
                            .inner
                            .executed_amount
                            .checked_mul(adjusted_sell_price)?
                            .checked_div(adjusted_buy_price)?;
                        let sell_amount_needed_with_ucp = received_buy_amount
                            .checked_mul(uniform_buy_price)?
                            .checked_div(uniform_sell_price)?;
                        trade
                            .inner
                            .executed_amount
                            .checked_sub(sell_amount_needed_with_ucp)?
                    }
                };
                (FeeKind::Surplus, fee)
            }
        };

        // converts the fee which is denominated in `sell_token` to the native token.
        tracing::trace!(?fee, "fee before conversion to native token");
        let native =
            external_prices.try_get_native_amount(*sell_token, u256_to_big_rational(&fee))?;
        tracing::trace!(?native, "fee after conversion to native token");

        Some(Fees {
            order: trade.order_uid,
            kind,
            sell: fee,
            native: big_rational_to_u256(&native).ok()?,
        })
    }
}

/// Can be populated multiple times for the same order (partially fillable
/// orders)
#[derive(Debug)]
pub struct Fees {
    /// The UID of the order associated with these fees.
    pub order: OrderUid,
    /// The type of fee that was executed.
    pub kind: FeeKind,
    /// The executed fees in the sell token.
    pub sell: U256,
    /// The executed fees in the native token.
    pub native: U256,
}

impl Fees {
    /// Get the surplus fee for this order.
    pub fn executed_surplus_fee(&self) -> Option<U256> {
        match self.kind {
            FeeKind::User => None,
            FeeKind::Surplus => Some(self.sell),
        }
    }
}

#[derive(Debug)]
pub enum FeeKind {
    User,
    Surplus,
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
    use {super::*, shared::addr, std::collections::BTreeMap};

    const MAINNET_DOMAIN_SEPARATOR: DomainSeparator = DomainSeparator(hex_literal::hex!(
        "c078f884a2676e1345748b1feace7b0abee5d00ecadb6e574dcdd109a63e8943"
    ));

    fn total_fee(fees: Vec<Fees>) -> U256 {
        fees.iter().fold(0.into(), |acc, fee| acc + fee.native)
    }

    fn order_executions(fees: Vec<Fees>) -> Vec<Fees> {
        fees.into_iter()
            .filter_map(|fee| match fee.kind {
                FeeKind::User => None,
                FeeKind::Surplus => Some(fee),
            })
            .collect()
    }

    #[test]
    fn total_surplus_test() {
        // transaction hash:
        // 0x4ed25533ae840fa36951c670b1535265977491b8c4db38d6fe3b2cffe3dad298

        // From solver competition table:

        // external prices (auction values):
        // 0x0f2d719407fdbeff09d87557abb7232601fd9f29: 773763471505852
        // 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48: 596635491559324261891964928
        // 0xdac17f958d2ee523a2206206994597c13d831ec7: 596703190526849003475173376
        // 0xf4d2888d29d722226fafa5d9b24f9164c092421e: 130282568907757

        // surplus: 33350701806766732

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
            000000000123432"
        );
        let settlement = DecodedSettlement::new(&call_data, &MAINNET_DOMAIN_SEPARATOR).unwrap();

        //calculate surplus
        let auction_external_prices = BTreeMap::from([
            (
                addr!("0f2d719407fdbeff09d87557abb7232601fd9f29"),
                U256::from(773763471505852u128),
            ),
            (
                addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                U256::from(596635491559324261891964928u128),
            ),
            (
                addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                U256::from(596703190526849003475173376u128),
            ),
            (
                addr!("f4d2888d29d722226fafa5d9b24f9164c092421e"),
                U256::from(130282568907757u128),
            ),
        ]);
        let native_token = addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let external_prices =
            ExternalPrices::try_from_auction_prices(native_token, auction_external_prices).unwrap();
        let surplus = settlement
            .total_surplus(&external_prices, Default::default())
            .to_f64_lossy(); // to_f64_lossy() to mimic what happens when value is saved for solver
                             // competition
        assert_eq!(surplus, 33350701806766732.);
    }

    #[test]
    fn total_fees_test() {
        // transaction hash:
        // 0x8f39bb793d3beac9aa944c5cc23e3e8677f639bdf87c2df9eb869a2875a8df7a

        // From solver competition table:

        // external prices (auction values):
        // 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48: 427705391752968402072764416
        // 0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee: 1000000000000000000

        // fees: 1234567890

        let call_data = hex_literal::hex!(
            "13d79a0b
            0000000000000000000000000000000000000000000000000000000000000080
            00000000000000000000000000000000000000000000000000000000000000e0
            0000000000000000000000000000000000000000000000000000000000000140
            0000000000000000000000000000000000000000000000000000000000000360
            0000000000000000000000000000000000000000000000000000000000000002
            000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
            000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
            0000000000000000000000000000000000000000000000000000000000000002
            00000000000000000000000000000000000000000000000008253cda5372fb00
            00000000000000000000000000000000000000000000000000000000515289a8
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000020
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000001daad45bafadf7c1fb26f6e9d10f2f559c3f6969
            00000000000000000000000000000000000000000000000000000000515289aa
            0000000000000000000000000000000000000000000000000818df7bcf8b291b
            0000000000000000000000000000000000000000000000000000000065ba1711
            f41dcea54d3ab11e7ad733f155df3d65d9afeacf3e73b5127fa55290ad8fdcbb
            0000000000000000000000000000000000000000000000000000000000b834f5
            0000000000000000000000000000000000000000000000000000000000000000
            00000000000000000000000000000000000000000000000000000000515289aa
            0000000000000000000000000000000000000000000000000000000000000160
            0000000000000000000000000000000000000000000000000000000000000041
            0914011870ad1446accb59cec344b3f5f5d4949e12ae7f37ab84d258bc69640d
            098bacc990d6e6d3f2c40ab0271e4aba15c6aef2fb454a368ef7a94d7c1e0f8b
            1c00000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000320
            0000000000000000000000000000000000000000000000000000000000000420
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000020
            00000000000000000000000001dcb88678aedd0c4cc9552b20f4718550250574
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            00000000000000000000000000000000000000000000000000000000000001e4
            760f2a0b00000000000000000000000000000000000000000000000000000000
            0000002000000000000000000000000000000000000000000000000000000000
            0000000100000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce
            3606eb4800000000000000000000000000000000000000000000000000000000
            0000006000000000000000000000000000000000000000000000000000000000
            0001388000000000000000000000000000000000000000000000000000000000
            000000e4d505accf0000000000000000000000001daad45bafadf7c1fb26f6e9
            d10f2f559c3f6969000000000000000000000000c92e8bdf79f0507f65a392b0
            ab4667716bfe0110ffffffffffffffffffffffffffffffffffffffffffffffff
            ffffffffffffffff000000000000000000000000000000000000000000000000
            000000006f21b76b000000000000000000000000000000000000000000000000
            000000000000001b411a84abb5867375378781ae7d25e93a6f49d7d2e1beed2d
            7a329e79234674984a58fb81c64d5258ebe6928e88f447d0bdbff6c5b6a6313a
            2ec93aede5d73954000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000020
            000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000024
            2e1a7d4d00000000000000000000000000000000000000000000000008253cda
            86bb7c4900000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000
            00000000007ff044"
        )
        .to_vec();
        let settlement = DecodedSettlement::new(&call_data, &MAINNET_DOMAIN_SEPARATOR).unwrap();

        //calculate fees
        let auction_external_prices = BTreeMap::from([
            (
                addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                U256::from(427705391752968402072764416u128),
            ),
            (
                addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
                U256::from(1000000000000000000u128),
            ),
        ]);
        let native_token = addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let external_prices =
            ExternalPrices::try_from_auction_prices(native_token, auction_external_prices).unwrap();

        let fees = settlement.all_fees(&external_prices);
        let fee = total_fee(fees).to_f64_lossy(); // to_f64_lossy() to mimic what happens when value is saved for solver
                                                  // competition
        assert_eq!(fee, 5163336903917741.);
    }

    #[test]
    fn total_fees_test_partial_limit_order() {
        // transaction hash:
        // 0x00e0e45ccc01b1bc99350444742cf5b4701d0c3eb85bc8c8f60a07e1e8cc4a36

        // From solver competition table:

        // external prices (auction values):
        // 0xba386a4ca26b85fd057ab1ef86e3dc7bdeb5ce70: 8302940
        // 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2: 1000000000000000000

        // fees: 3768095572151423

        let call_data = hex_literal::hex!(
            "13d79a0b
            0000000000000000000000000000000000000000000000000000000000000080
            0000000000000000000000000000000000000000000000000000000000000120
            00000000000000000000000000000000000000000000000000000000000001c0
            00000000000000000000000000000000000000000000000000000000000003e0
            0000000000000000000000000000000000000000000000000000000000000004
            000000000000000000000000ba386a4ca26b85fd057ab1ef86e3dc7bdeb5ce70
            000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
            000000000000000000000000ba386a4ca26b85fd057ab1ef86e3dc7bdeb5ce70
            000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
            0000000000000000000000000000000000000000000000000000000000000004
            000000000000000000000000000000000000000000000000000000000083732b
            0000000000000000000000000000000000000000000000000de0b6b3a7640000
            0000000000000000000000000000000000000000000000000ff962d1e3a803f9
            0000000000000000000000000000000000000001b133ca2607cfe842f8f4c8ef
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000020
            0000000000000000000000000000000000000000000000000000000000000002
            0000000000000000000000000000000000000000000000000000000000000003
            0000000000000000000000006c7f534c81dfedf90c9e42effb410a44e4f8ef10
            0000000000000000000000000000000000000002863c1f5cdae42f9540000000
            00000000000000000000000000000000000000000000000017979cfe362a0000
            0000000000000000000000000000000000000000000000000000000064690e05
            c1164815465bff632c198b8455e9a421c07e8ce426c8cd1b59eef7b305b8ca90
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000002
            0000000000000000000000000000000000000001b133ca2607cfe842f8f4c8ef
            0000000000000000000000000000000000000000000000000000000000000160
            0000000000000000000000000000000000000000000000000000000000000041
            f8ad81db7333b891f88527d100a06f23ff4d7859c66ddd71514291379deb8ff6
            60f4fb2a24173eaac5fad2a124823e968686e39467c7f3054c13c4b70980cc1a
            1c00000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000080
            0000000000000000000000000000000000000000000000000000000000000260
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000020
            0000000000000000000000007a250d5630b4cf539739df2c5dacb4c659f2488d
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000104
            8803dbee0000000000000000000000000000000000000000000000000ff962d4
            52d79e2a0000000000000000000000000000000000000001b02aeadbd4ac2231
            68f3b31200000000000000000000000000000000000000000000000000000000
            000000a00000000000000000000000009008d19f58aabd9ed0d60971565aa851
            0560ab41ffffffffffffffffffffffffffffffffffffffffffffffffffffffff
            ffffffff00000000000000000000000000000000000000000000000000000000
            00000002000000000000000000000000ba386a4ca26b85fd057ab1ef86e3dc7b
            deb5ce70000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead908
            3c756cc200000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000"
        );
        let settlement = DecodedSettlement::new(&call_data, &MAINNET_DOMAIN_SEPARATOR).unwrap();

        //calculate fees
        let auction_external_prices = BTreeMap::from([
            (
                addr!("ba386a4ca26b85fd057ab1ef86e3dc7bdeb5ce70"),
                U256::from(8302940),
            ),
            (
                addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                U256::from(1000000000000000000u128),
            ),
        ]);
        let native_token = addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let external_prices =
            ExternalPrices::try_from_auction_prices(native_token, auction_external_prices).unwrap();

        let fees = settlement.all_fees(&external_prices);
        let fee = total_fee(fees).to_f64_lossy(); // to_f64_lossy() to mimic what happens when value is saved for solver
                                                  // competition
        assert_eq!(fee, 3768095572151424.);
    }

    #[test]
    fn execution_amount_does_not_matter_for_fok_orders() {
        // transaction hash:
        // 0xd2a3b85244bee6043f740ce774bc72ba271b890c4aa939ebe3d859afef445d99

        // From solver competition table:

        // external prices (auction values):
        // 0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee: 1000000000000000000
        // 0xf88baf18fab7e330fa0c4f83949e23f52fececce: 29428019732094

        // fees: 688868232097089454080

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
        let settlement = DecodedSettlement::new(&call_data, &MAINNET_DOMAIN_SEPARATOR).unwrap();

        //calculate fees
        let auction_external_prices = BTreeMap::from([
            (
                addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
                U256::from(1000000000000000000u128),
            ),
            (
                addr!("f88baf18fab7e330fa0c4f83949e23f52fececce"),
                U256::from(29428019732094u128),
            ),
        ]);
        let native_token = addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let external_prices =
            ExternalPrices::try_from_auction_prices(native_token, auction_external_prices).unwrap();

        let fees = settlement.all_fees(&external_prices);
        let fee = total_fee(fees).to_f64_lossy(); // to_f64_lossy() to mimic what happens when value is saved for solver
                                                  // competition
        assert_eq!(fee, 20272027926965858.);
    }

    #[test]
    fn decodes_metadata() {
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
        )
        .to_vec();
        let original = DecodedSettlement::new(&call_data, &MAINNET_DOMAIN_SEPARATOR).unwrap();

        // If not enough call data got appended we parse it like it didn't have any
        // Not enough metadata appended to the calldata.
        let metadata = [42; DecodedSettlement::META_DATA_LEN - 1];
        let with_metadata = [call_data.clone(), metadata.to_vec()].concat();
        assert_eq!(
            original,
            DecodedSettlement::new(&with_metadata, &MAINNET_DOMAIN_SEPARATOR).unwrap()
        );

        // Same if too much metadata gets added.
        let metadata = [42; DecodedSettlement::META_DATA_LEN];
        let with_metadata = [call_data.clone(), vec![100], metadata.to_vec()].concat();
        assert_eq!(
            original,
            DecodedSettlement::new(&with_metadata, &MAINNET_DOMAIN_SEPARATOR).unwrap()
        );

        // If we add exactly the expected number of bytes we can parse the metadata.
        let metadata = [42; DecodedSettlement::META_DATA_LEN];
        let with_metadata = [call_data, metadata.to_vec()].concat();
        let with_metadata =
            DecodedSettlement::new(&with_metadata, &MAINNET_DOMAIN_SEPARATOR).unwrap();
        assert_eq!(with_metadata.metadata, Some(Bytes(metadata)));

        // Content of the remaining fields is identical to the original
        let metadata_removed_again = DecodedSettlement {
            metadata: None,
            ..with_metadata
        };
        assert_eq!(original, metadata_removed_again);
    }

    #[test]
    fn test_signature_collision() {
        // 0xd881e90f4afb020d92b8fa1b4931d2352aab4179e4f8d9a4aeafd01ebc75f808
        // Two FOK orders with identical signatures led to incorrect fee computation
        let call_data = hex_literal::hex!(
            "13d79a0b
            0000000000000000000000000000000000000000000000000000000000000080
            00000000000000000000000000000000000000000000000000000000000001e0
            0000000000000000000000000000000000000000000000000000000000000340
            00000000000000000000000000000000000000000000000000000000000008a0
            000000000000000000000000000000000000000000000000000000000000000a
            00000000000000000000000031429d1856ad1377a8a0079410b297e1a9e214c2
            000000000000000000000000d533a949740bb3306d119cc777fa900ba034cd52
            000000000000000000000000da816459f1ab5631232fe5e97a05bbbb94970c95
            000000000000000000000000fbeb78a723b8087fd2ea7ef1afec93d35e8bed42
            00000000000000000000000031429d1856ad1377a8a0079410b297e1a9e214c2
            000000000000000000000000da816459f1ab5631232fe5e97a05bbbb94970c95
            000000000000000000000000fbeb78a723b8087fd2ea7ef1afec93d35e8bed42
            000000000000000000000000da816459f1ab5631232fe5e97a05bbbb94970c95
            000000000000000000000000d533a949740bb3306d119cc777fa900ba034cd52
            000000000000000000000000da816459f1ab5631232fe5e97a05bbbb94970c95
            000000000000000000000000000000000000000000000000000000000000000a
            0000000000000000000000000000000c0b03c81119b84f43f1c522cebd1f758c
            0000000000000000000000000000010b4252cd1c6991d374aa0367a666a03dda
            000000000000000000000000000001b69886af6eb7b1edaae84948d71fd80000
            000000000000000000000000000009c99241962dc50ca31d13f4d868071f0dce
            00000000000000000000000000000000000000000000028c55a5804e2911a0a8
            000000000000000000000000000000000000000000005d5558f7bd2b69f6ef02
            0000000000000000000000000000000000000000000001e0459e08bd75a757d1
            0000000000000000000000000000000000000000000000547a054e6df9e58f6e
            00000000000000000000000000000000000000000000067067b5c2bc38aebeb7
            000000000000000000000000000000000000000000000a968163f0a57b000000
            0000000000000000000000000000000000000000000000000000000000000003
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000200
            00000000000000000000000000000000000000000000000000000000000003a0
            0000000000000000000000000000000000000000000000000000000000000004
            0000000000000000000000000000000000000000000000000000000000000005
            00000000000000000000000093a62da5a14c80f265dabc077fcee437b1a0efde
            000000000000000000000000000000000000000000005d5558f7bd2b69f6ef02
            00000000000000000000000000000000000000000000028bd9aa06001dc7352a
            0000000000000000000000000000000000000000000000000000000065782608
            2b8694ed30082129598720860e8e972f07aa10d9b81cae16ca0e2cfb24743e24
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            000000000000000000000000000000000000000000005d5558f7bd2b69f6ef02
            0000000000000000000000000000000000000000000000000000000000000160
            0000000000000000000000000000000000000000000000000000000000000014
            c001d00d425fa92c4f840baa8f1e0c27c4297a0b000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000006
            0000000000000000000000000000000000000000000000000000000000000007
            00000000000000000000000093a62da5a14c80f265dabc077fcee437b1a0efde
            0000000000000000000000000000000000000000000000547a054e6df9e58f6e
            0000000000000000000000000000000000000000000001e000aef194b46081ed
            0000000000000000000000000000000000000000000000000000000065782608
            2b8694ed30082129598720860e8e972f07aa10d9b81cae16ca0e2cfb24743e24
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000547a054e6df9e58f6e
            0000000000000000000000000000000000000000000000000000000000000160
            0000000000000000000000000000000000000000000000000000000000000014
            c001d00d425fa92c4f840baa8f1e0c27c4297a0b000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000008
            0000000000000000000000000000000000000000000000000000000000000009
            00000000000000000000000093a62da5a14c80f265dabc077fcee437b1a0efde
            000000000000000000000000000000000000000000000a968163f0a57b000000
            000000000000000000000000000000000000000000000667b861097b602a7162
            0000000000000000000000000000000000000000000000000000000065782608
            2b8694ed30082129598720860e8e972f07aa10d9b81cae16ca0e2cfb24743e24
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            000000000000000000000000000000000000000000000a968163f0a57b000000
            0000000000000000000000000000000000000000000000000000000000000160
            0000000000000000000000000000000000000000000000000000000000000014
            c001d00d425fa92c4f840baa8f1e0c27c4297a0b000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000080
            0000000000000000000000000000000000000000000000000000000000001360
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000004
            0000000000000000000000000000000000000000000000000000000000000080
            0000000000000000000000000000000000000000000000000000000000000160
            0000000000000000000000000000000000000000000000000000000000000240
            0000000000000000000000000000000000000000000000000000000000000320
            00000000000000000000000031429d1856ad1377a8a0079410b297e1a9e214c2
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000044
            a9059cbb000000000000000000000000b634316e06cc0b358437cbadd4dc94f1
            d3a92b3b000000000000000000000000000000000000000000005ccd6ab98ff5
            8eb1005b00000000000000000000000000000000000000000000000000000000
            000000000000000000000000d533a949740bb3306d119cc777fa900ba034cd52
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000044
            a9059cbb000000000000000000000000b634316e06cc0b358437cbadd4dc94f1
            d3a92b3b000000000000000000000000000000000000000000000a912dc1b812
            70152dfc00000000000000000000000000000000000000000000000000000000
            000000000000000000000000fbeb78a723b8087fd2ea7ef1afec93d35e8bed42
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000044
            a9059cbb000000000000000000000000b634316e06cc0b358437cbadd4dc94f1
            d3a92b3b000000000000000000000000000000000000000000000054121cab49
            c2a66e8100000000000000000000000000000000000000000000000000000000
            000000000000000000000000b634316e06cc0b358437cbadd4dc94f1d3a92b3b
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000060
            0000000000000000000000000000000000000000000000000000000000000f04
            de792d5f00000000000000000000000000000000000000000000000000000000
            0000004000000000000000000000000000000000000000000000000000000000
            000001c000000000000000000000000000000000000000000000000000000000
            0000000b095ea7b3010001ffffffffff31429d1856ad1377a8a0079410b297e1
            a9e214c22473d02f40ffffffffffff09fef040b55e74b8080e4076575e258453
            e785162e0002030402050607018889ffffffffffffffffffffffffffffffffff
            ffffffff2473d02f40ffffffffffff0cfef040b55e74b8080e4076575e258453
            e785162e00020a04020506070b888cffffffffffffffffffffffffffffffffff
            ffffffff2e1a7d4d010dffffffffff0dfbeb78a723b8087fd2ea7ef1afec93d3
            5e8bed42095ea7b301000dffffffffff1f9840a85d5af5bf1d1762f925bdaddc
            4201f984e449022e010d068effffff0d1111111254eeb25477b68fb85ed929f7
            3a960582d33fb53c02090d0cffffff0dfef040b55e74b8080e4076575e258453
            e785162eb6b55f25010dffffffffffffda816459f1ab5631232fe5e97a05bbbb
            94970c95d1660f99000f1011ffffffff1b45a86e58b97df309bae0e6c4bbcd40
            f5a23d5400000000000000000000000000000000000000000000000000000000
            0000001200000000000000000000000000000000000000000000000000000000
            0000024000000000000000000000000000000000000000000000000000000000
            0000028000000000000000000000000000000000000000000000000000000000
            000002c000000000000000000000000000000000000000000000000000000000
            0000030000000000000000000000000000000000000000000000000000000000
            0000034000000000000000000000000000000000000000000000000000000000
            0000038000000000000000000000000000000000000000000000000000000000
            000003c000000000000000000000000000000000000000000000000000000000
            0000040000000000000000000000000000000000000000000000000000000000
            0000044000000000000000000000000000000000000000000000000000000000
            000004a000000000000000000000000000000000000000000000000000000000
            0000082000000000000000000000000000000000000000000000000000000000
            0000086000000000000000000000000000000000000000000000000000000000
            000008a000000000000000000000000000000000000000000000000000000000
            00000ba000000000000000000000000000000000000000000000000000000000
            00000be000000000000000000000000000000000000000000000000000000000
            00000c6000000000000000000000000000000000000000000000000000000000
            00000ca000000000000000000000000000000000000000000000000000000000
            00000ce000000000000000000000000000000000000000000000000000000000
            000000200000000000000000000000001111111254eeb25477b68fb85ed929f7
            3a96058200000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000000000000000000000005ccd6ab98ff5
            8eb1005b00000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000e37e799d5077682fa0a244d46e5649f7
            1457bd0900000000000000000000000000000000000000000000000000000000
            0000002000000000000000000000000031429d1856ad1377a8a0079410b297e1
            a9e214c200000000000000000000000000000000000000000000000000000000
            000000200000000000000000000000006b175474e89094c44da98b954eedeac4
            95271d0f00000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000b634316e06cc0b358437cbadd4dc94f1
            d3a92b3b00000000000000000000000000000000000000000000000000000000
            0000002000000000000000000000000000000000000000000000000000000000
            0000000100000000000000000000000000000000000000000000000000000000
            0000002000000000000000000000000000000000000000000000000000000000
            0000000400000000000000000000000000000000000000000000000000000000
            0000004000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000
            0000036000000000000000000000000000000000000000000000000000000000
            0000032b00000000000000000000000000000000000000030d0002df00029500
            001a0020d6bdbf7831429d1856ad1377a8a0079410b297e1a9e214c200a0c9e7
            5c48000000000000000006040000000000000000000000000000000000000000
            0000000000024d00011000a007e5c0d200000000000000000000000000000000
            00000000000000ec0000b200004f02a000000000000000000000000000000000
            00000000000000000000000000000001ee63c1e50151c2841333fbbab53b7c2c
            442cc265bf16430d6d31429d1856ad1377a8a0079410b297e1a9e214c202a000
            00000000000000000000000000000000000000000000000000000000000001ee
            63c1e581c7bbec68d12a0d1830360f8ec58fa599ba1b0e9bc02aaa39b223fe8d
            0a0e5c4f27ead9083c756cc23058ef90929cb8180174d74c507176cca6835d73
            40203058ef90929cb8180174d74c507176cca6835d73dd93f59a000000000000
            000000000000e37e799d5077682fa0a244d46e5649f71457bd0900a007e5c0d2
            0000000000000000000000000000000000000000000001190000ca00007b0c20
            31429d1856ad1377a8a0079410b297e1a9e214c21f4c763bde1d4832b3ea0640
            e66da00b988313556ae4071118002dc6c01f4c763bde1d4832b3ea0640e66da0
            0b98831355000000000000000000000000000000000000000000000000000000
            000000000131429d1856ad1377a8a0079410b297e1a9e214c202a00000000000
            000000000000000000000000000000000000000000000000000001ee63c1e501
            735a26a57a0a0069dfabd41595a970faf5e1ee8b1a7e4e63778b4f12a199c062
            f3efdd288afcbce802a000000000000000000000000000000000000000000000
            00000000000000000001ee63c1e5005777d92f208679db4b9778590fa3cab3ac
            9e2168a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4800a0f2fa6b666b1754
            74e89094c44da98b954eedeac495271d0f000000000000000000000000000000
            0000000000000002c483c53cc556d8fbfa00000000000000004544e3f760f00f
            e780a06c4eca276b175474e89094c44da98b954eedeac495271d0f1111111254
            eeb25477b68fb85ed929f73a9605820000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000d533a949740bb3306d119cc777fa900b
            a034cd5200000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000000000000000000000000a912dc1b812
            70152dfc00000000000000000000000000000000000000000000000000000000
            000002e000000000000000000000000000000000000000000000000000000000
            000002bf0000000000000000000000000000000000000002a100027300022900
            001a0020d6bdbf78d533a949740bb3306d119cc777fa900ba034cd5200a007e5
            c0d20000000000000000000000000000000000000000000001eb00019c00014d
            00a0c9e75c4800000000000000001e1400000000000000000000000000000000
            000000000000000000011f00004f02a000000000000000000000000000000000
            00000000000000000000000000000001ee63c1e500919fa96e88d67499339577
            fa202345436bcdaf79d533a949740bb3306d119cc777fa900ba034cd5251204e
            bdf703948ddcea3b11f675b4d1fba9d2414a14d533a949740bb3306d119cc777
            fa900ba034cd520044394747c500000000000000000000000000000000000000
            0000000000000000000000000200000000000000000000000000000000000000
            0000000000000000000000000100000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000100000000000000000000000000000000000000
            0000000000000000000000000002a00000000000000000000000000000000000
            000000000000000000000000000001ee63c1e50088e6a0c2ddd26feeb64f039a
            2c41296fcb3f5640c02aaa39b223fe8d0a0e5c4f27ead9083c756cc202a00000
            000000000000000000000000000000000000000000000000000000000001ee63
            c1e5005777d92f208679db4b9778590fa3cab3ac9e2168a0b86991c6218b36c1
            d19d4a2e9eb0ce3606eb4800a0f2fa6b666b175474e89094c44da98b954eedea
            c495271d0f0000000000000000000000000000000000000000000006f76d5072
            3b9cd23ab800000000000000004544e3f760f00fe780a06c4eca276b175474e8
            9094c44da98b954eedeac495271d0f1111111254eeb25477b68fb85ed929f73a
            9605820000000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000000000000000000000000054121cab49
            c2a66e8100000000000000000000000000000000000000000000000000000000
            0000006000000000000000000000000000000000000000000000000000000000
            000000020000000000000000000000001d42064fc4beb5f8aaf85f4617ae8b3b
            5b8bd80180000000000000000000000060594a405d53811d3bc4766596efd80f
            d545a27000000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000da816459f1ab5631232fe5e97a05bbbb
            94970c9500000000000000000000000000000000000000000000000000000000
            000000200000000000000000000000009008d19f58aabd9ed0d60971565aa851
            0560ab4100000000000000000000000000000000000000000000000000000000
            00000020000000000000000000000000000000000000000000000add02f94bc7
            d767b73000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000
            00000000007c3910"
        )
        .to_vec();

        let decoded = DecodedSettlement::new(&call_data, &MAINNET_DOMAIN_SEPARATOR).unwrap();
        let auction_external_prices = BTreeMap::from([
            (
                addr!("31429d1856ad1377a8a0079410b297e1a9e214c2"),
                U256::from(1000000000000000000u128),
            ),
            (
                addr!("d533a949740bb3306d119cc777fa900ba034cd52"),
                U256::from(1000000000000000000u128),
            ),
            (
                addr!("da816459f1ab5631232fe5e97a05bbbb94970c95"),
                U256::from(1000000000000000000u128),
            ),
            (
                addr!("fbeb78a723b8087fd2ea7ef1afec93d35e8bed42"),
                U256::from(1000000000000000000u128),
            ),
        ]);
        let native_token = addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let external_prices =
            ExternalPrices::try_from_auction_prices(native_token, auction_external_prices).unwrap();

        let fees = decoded.all_fees(&external_prices);
        let fees = order_executions(fees);
        assert_eq!(fees[1].sell, 7487413756444483822u128.into());
    }
}

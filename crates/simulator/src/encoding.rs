use {
    alloy_primitives::{Address, B256, Bytes, U256},
    alloy_sol_types::SolCall,
    app_data::AppDataHash,
    contracts::alloy::GPv2Settlement,
    derive_more::Debug,
    model::{
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::{Signature, SigningScheme},
    },
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

pub type EncodedTrade = (
    U256,    // sellTokenIndex
    U256,    // buyTokenIndex
    Address, // receiver
    U256,    // sellAmount
    U256,    // buyAmount
    u32,     // validTo
    B256,    // appData
    U256,    // feeAmount
    U256,    // flags
    U256,    // executedAmount
    Bytes,   // signature
);

// TODO: Change Vec into VecDeque for easy sandwitching of custom pre, main,
// post interaction at the callsite.
// This can't work elegantly until `extend_front` of VecDeque becomes stabilized
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Interactions {
    pub pre: Vec<EncodedInteraction>,
    pub main: Vec<EncodedInteraction>,
    pub post: Vec<EncodedInteraction>,
}

impl Interactions {
    pub fn into_array(self) -> [Vec<EncodedInteraction>; 3] {
        [self.pre, self.main, self.post]
    }
}

impl IntoIterator for Interactions {
    type IntoIter = std::array::IntoIter<Vec<EncodedInteraction>, 3>;
    type Item = Vec<EncodedInteraction>;

    fn into_iter(self) -> Self::IntoIter {
        [self.pre, self.main, self.post].into_iter()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodedSettlement {
    pub tokens: Vec<Address>,
    pub clearing_prices: Vec<U256>,
    pub trades: Vec<EncodedTrade>,
    pub interactions: Interactions,
}

impl EncodedSettlement {
    pub fn into_settle_call(&self) -> Bytes {
        GPv2Settlement::GPv2Settlement::settleCall {
            tokens: self.tokens.clone(),
            clearingPrices: self.clearing_prices.clone(),
            interactions: self.interactions.clone().into_array().map(|interactions| {
                interactions
                    .into_iter()
                    .map(|i| GPv2Settlement::GPv2Interaction::Data {
                        target: i.0,
                        value: i.1,
                        callData: i.2.0.into(),
                    })
                    .collect()
            }),
            trades: self
                .trades
                .iter()
                .map(|t| GPv2Settlement::GPv2Trade::Data {
                    sellTokenIndex: t.0,
                    buyTokenIndex: t.1,
                    receiver: t.2,
                    sellAmount: t.3,
                    buyAmount: t.4,
                    validTo: t.5,
                    appData: t.6,
                    feeAmount: t.7,
                    flags: t.8,
                    executedAmount: t.9,
                    signature: t.10.clone(),
                })
                .collect(),
        }
        .abi_encode()
        .into()
    }
}

pub type EncodedInteraction = (
    Address, // target
    U256,    // value
    Bytes,   // callData
);

#[serde_as]
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JitOrder {
    pub buy_token: Address,
    pub sell_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed_amount: U256,
    pub receiver: Address,
    pub valid_to: u32,
    pub app_data: AppDataHash,
    pub side: Side,
    pub partially_fillable: bool,
    pub sell_token_source: SellTokenSource,
    pub buy_token_destination: BuyTokenDestination,
    #[serde_as(as = "serde_ext::Hex")]
    pub signature: Vec<u8>,
    pub signing_scheme: SigningScheme,
}

#[serde_as]
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}

/// Creates the data which the smart contract's `decodeTrade` expects.
pub fn encode_trade(
    order: &OrderData,
    signature: &Signature,
    owner: Address,
    sell_token_index: usize,
    buy_token_index: usize,
    executed_amount: U256,
) -> EncodedTrade {
    (
        U256::from(sell_token_index),
        U256::from(buy_token_index),
        order.receiver.unwrap_or(Address::ZERO),
        order.sell_amount,
        order.buy_amount,
        order.valid_to,
        B256::new(order.app_data.0),
        order.fee_amount,
        order_flags(order, signature),
        executed_amount,
        Bytes::from(signature.encode_for_settlement(owner)),
    )
}

fn order_flags(order: &OrderData, signature: &Signature) -> U256 {
    let mut result = 0u8;
    // The kind is encoded as 1 bit in position 0.
    result |= match order.kind {
        OrderKind::Sell => 0b0,
        OrderKind::Buy => 0b1,
    };
    // The order fill kind is encoded as 1 bit in position 1.
    result |= (order.partially_fillable as u8) << 1;
    // The order sell token balance is encoded as 2 bits in position 2.
    result |= match order.sell_token_balance {
        SellTokenSource::Erc20 => 0b00,
        SellTokenSource::External => 0b10,
        SellTokenSource::Internal => 0b11,
    } << 2;
    // The order buy token balance is encoded as 1 bit in position 4.
    result |= match order.buy_token_balance {
        BuyTokenDestination::Erc20 => 0b0,
        BuyTokenDestination::Internal => 0b1,
    } << 4;
    // The signing scheme is encoded as a 2 bits in position 5.
    result |= match signature.scheme() {
        SigningScheme::Eip712 => 0b00,
        SigningScheme::EthSign => 0b01,
        SigningScheme::Eip1271 => 0b10,
        SigningScheme::PreSign => 0b11,
    } << 5;
    U256::from(result)
}

/// Data for a raw GPv2 interaction.
#[derive(Clone, PartialEq, Eq, Hash, Default, Serialize, Debug)]
pub struct Interaction {
    pub target: Address,
    pub value: U256,
    #[debug("{}", const_hex::encode_prefixed::<&[u8]>(data.as_ref()))]
    pub data: Vec<u8>,
}

pub trait InteractionEncoding {
    fn encode(&self) -> EncodedInteraction;
}

impl Interaction {
    pub fn to_interaction_data(&self) -> InteractionData {
        InteractionData {
            target: self.target,
            value: self.value,
            call_data: self.data.clone(),
        }
    }
}

impl InteractionEncoding for Interaction {
    fn encode(&self) -> EncodedInteraction {
        (
            self.target,
            self.value,
            Bytes::copy_from_slice(self.data.as_slice()),
        )
    }
}

impl InteractionEncoding for InteractionData {
    fn encode(&self) -> EncodedInteraction {
        (
            self.target,
            self.value,
            Bytes::copy_from_slice(&self.call_data),
        )
    }
}

impl From<InteractionData> for Interaction {
    fn from(interaction: InteractionData) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            data: interaction.call_data,
        }
    }
}

pub fn encode_interactions<'a>(
    interactions: impl IntoIterator<Item = &'a Interaction>,
) -> Vec<EncodedInteraction> {
    interactions.into_iter().map(|i| i.encode()).collect()
}

#[derive(Clone, Debug)]
pub struct WrapperCall {
    pub address: Address,
    pub data: Bytes,
}

/// Encodes a settlement transaction that uses wrapper contracts.
///
/// Takes the base settlement calldata and wraps it in a wrappedSettleCall
/// with encoded wrapper metadata. Since wrappers are a chain, the wrapper
/// address to call is also processed by this function.
///
/// Returns (first_wrapper_address, wrapped_calldata)
pub fn encode_wrapper_settlement(
    wrappers: &[WrapperCall],
    settle_calldata: Bytes,
) -> Option<(Address, Bytes)> {
    if wrappers.is_empty() {
        return None;
    };
    let wrapper_data = encode_wrapper_data(wrappers);

    // Create wrappedSettleCall
    let calldata = contracts::alloy::ICowWrapper::ICowWrapper::wrappedSettleCall {
        settleData: settle_calldata,
        wrapperData: wrapper_data,
    }
    .abi_encode();

    Some((wrappers[0].address, calldata.into()))
}

/// Encodes wrapper metadata for wrapper settlement calls.
/// As wrappers are called, each wrapper reads from wrapper calldata and
/// consumes only their needed portion (however much data that is). Once wrapper
/// is ready to call the settlement contract (or downstream wrapper) it calls
/// the _internalSettle function provided in the CowWrapper abstract contract
///
/// Generally wrappers are encoded with a pair of Address (20 bytes) and then
/// calldata (u16 length + data itself).
///
/// Since the first wrapper's address is the target of the transaction, it is
/// not encoded.
///
/// The encoding format thus is:
/// - The calldata of the first wrapper.
/// - The address and calldata for each subsequent wrapper
///
/// Example: Encoding of 2 wrapper calls, the wrappers are named A, B and are
/// called in the order A -> B
///
/// | A calldata length | A calldata | B address | B calldata length | B calldata |
/// | u16               | &[u8]      | [u8; 20]  | u16               | &[u8]      |
///
/// Any additional wrappers will follow the same scheme: (address, length,
/// calldata)
///
/// More information about wrapper encoding:
/// https://docs.cow.fi/cow-protocol/integrate/wrappers#manual-encoding
pub fn encode_wrapper_data(wrappers: &[WrapperCall]) -> Bytes {
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

    wrapper_data.into()
}

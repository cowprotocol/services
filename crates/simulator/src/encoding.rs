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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodedSettlement {
    pub tokens: Vec<Address>,
    pub clearing_prices: Vec<U256>,
    pub trades: Vec<EncodedTrade>,
    pub interactions: [Vec<EncodedInteraction>; 3],
}

impl EncodedSettlement {
    pub fn into_settle_call(&self) -> Bytes {
        GPv2Settlement::GPv2Settlement::settleCall {
            tokens: self.tokens.clone(),
            clearingPrices: self.clearing_prices.clone(),
            interactions: self.interactions.clone().map(|interactions| {
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

impl Interaction {
    pub fn encode(&self) -> EncodedInteraction {
        (
            self.target,
            self.value,
            Bytes::copy_from_slice(self.data.as_slice()),
        )
    }

    pub fn to_interaction_data(&self) -> InteractionData {
        InteractionData {
            target: self.target,
            value: self.value,
            call_data: self.data.clone(),
        }
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

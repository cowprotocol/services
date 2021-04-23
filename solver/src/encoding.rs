use model::{
    order::{OrderCreation, OrderKind},
    SigningScheme,
};
use primitive_types::{H160, U256};

pub type EncodedTrade = (
    U256,     // sellTokenIndex
    U256,     // buyTokenIndex
    H160,     // receiver
    U256,     // sellAmount
    U256,     // buyAmount
    u32,      // validTo
    [u8; 32], // appData
    U256,     // feeAmount
    U256,     // flags
    U256,     // executedAmount
    Vec<u8>,  // signature
);

/// Creates the data which the smart contract's `decodeTrade` expects.
pub fn encode_trade(
    order: &OrderCreation,
    sell_token_index: usize,
    buy_token_index: usize,
    executed_amount: &U256,
) -> EncodedTrade {
    (
        sell_token_index.into(),
        buy_token_index.into(),
        order.receiver.unwrap_or_else(H160::zero),
        order.sell_amount,
        order.buy_amount,
        order.valid_to,
        order.app_data,
        order.fee_amount,
        order_flags(order),
        *executed_amount,
        order.signature.to_bytes().to_vec(),
    )
}

fn order_flags(order: &OrderCreation) -> U256 {
    let mut result = 0u8;
    result |= match order.kind {
        OrderKind::Sell => 0,
        OrderKind::Buy => 0b1,
    };
    if order.partially_fillable {
        result |= 0b10;
    };
    result |= match order.signing_scheme {
        SigningScheme::Eip712 => 0,
        SigningScheme::EthSign => 0b100,
    };
    result.into()
}

pub type EncodedInteraction = (
    H160,    // target
    U256,    // value
    Vec<u8>, // callData
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedSettlement {
    pub tokens: Vec<H160>,
    pub clearing_prices: Vec<U256>,
    pub trades: Vec<EncodedTrade>,
    pub interactions: [Vec<EncodedInteraction>; 3],
}

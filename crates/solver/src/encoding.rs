use ethcontract::Bytes;
use model::{
    order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
    signature::{Signature, SigningScheme},
};
use primitive_types::{H160, U256};

pub type EncodedTrade = (
    U256,            // sellTokenIndex
    U256,            // buyTokenIndex
    H160,            // receiver
    U256,            // sellAmount
    U256,            // buyAmount
    u32,             // validTo
    Bytes<[u8; 32]>, // appData
    U256,            // feeAmount
    U256,            // flags
    U256,            // executedAmount
    Bytes<Vec<u8>>,  // signature
);

/// Creates the data which the smart contract's `decodeTrade` expects.
pub fn encode_trade(
    order: &OrderData,
    signature: &Signature,
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
        Bytes(order.app_data.0),
        order.fee_amount,
        order_flags(order, signature),
        *executed_amount,
        Bytes(signature.to_bytes().to_vec()),
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
    result.into()
}

pub type EncodedInteraction = (
    H160,           // target
    U256,           // value
    Bytes<Vec<u8>>, // callData
);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodedSettlement {
    pub tokens: Vec<H160>,
    pub clearing_prices: Vec<U256>,
    pub trades: Vec<EncodedTrade>,
    pub interactions: [Vec<EncodedInteraction>; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_flag_permutations() {
        for (order, signature, flags) in &[
            (
                OrderData {
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    sell_token_balance: SellTokenSource::Erc20,
                    buy_token_balance: BuyTokenDestination::Erc20,
                    ..Default::default()
                },
                Signature::default_with(SigningScheme::Eip712),
                // ......0 - sell order
                // .....0. - fill-or-kill order
                // ...00.. - ERC20 sell token balance
                // ..0.... - ERC20 buy token balance
                // 00..... - EIP-712 signing scheme
                0b0000000,
            ),
            (
                OrderData {
                    kind: OrderKind::Sell,
                    partially_fillable: true,
                    sell_token_balance: SellTokenSource::Erc20,
                    buy_token_balance: BuyTokenDestination::Internal,
                    ..Default::default()
                },
                Signature::default_with(SigningScheme::Eip712),
                // ......0 - sell order
                // .....1. - partially fillable order
                // ...00.. - ERC20 sell token balance
                // ..1.... - Vault-internal buy token balance
                // 00..... - EIP-712 signing scheme
                0b0010010,
            ),
            (
                OrderData {
                    kind: OrderKind::Buy,
                    partially_fillable: false,
                    sell_token_balance: SellTokenSource::External,
                    buy_token_balance: BuyTokenDestination::Erc20,
                    ..Default::default()
                },
                Signature::default_with(SigningScheme::PreSign),
                // ......1 - buy order
                // .....0. - fill-or-kill order
                // ...10.. - Vault-external sell token balance
                // ..0.... - ERC20 buy token balance
                // 11..... - Pre-sign signing scheme
                0b1101001,
            ),
            (
                OrderData {
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    sell_token_balance: SellTokenSource::Internal,
                    buy_token_balance: BuyTokenDestination::Erc20,
                    ..Default::default()
                },
                Signature::default_with(SigningScheme::EthSign),
                // ......0 - sell order
                // .....0. - fill-or-kill order
                // ...11.. - Vault-internal sell token balance
                // ..0.... - ERC20 buy token balance
                // 01..... - Eth-sign signing scheme
                0b0101100,
            ),
            (
                OrderData {
                    kind: OrderKind::Buy,
                    partially_fillable: true,
                    sell_token_balance: SellTokenSource::Internal,
                    buy_token_balance: BuyTokenDestination::Internal,
                    ..Default::default()
                },
                Signature::default_with(SigningScheme::PreSign),
                // ......1 - buy order
                // .....1. - partially fillable order
                // ...11.. - Vault-internal sell token balance
                // ..1.... - Vault-internal buy token balance
                // 11..... - Pre-sign signing scheme
                0b1111111,
            ),
        ] {
            assert_eq!(order_flags(order, signature), U256::from(*flags));
        }
    }
}

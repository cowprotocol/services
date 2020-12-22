use std::convert::TryInto;

use model::{OrderCreation, OrderKind};
use primitive_types::U256;

pub const TRADE_STRIDE: usize = 206;

/// Creates the data which the smart contract's `decodeTrade` expects.
pub fn encode_trade(
    order: &OrderCreation,
    sell_token_index: u8,
    buy_token_index: u8,
    executed_amount: &U256,
    fee_discount: u16,
) -> [u8; TRADE_STRIDE] {
    let mut result = [0u8; TRADE_STRIDE];
    result[0] = sell_token_index;
    result[1] = buy_token_index;
    order.sell_amount.to_big_endian(&mut result[2..34]);
    order.buy_amount.to_big_endian(&mut result[34..66]);
    result[66..70].copy_from_slice(&order.valid_to.to_be_bytes());
    result[70..74].copy_from_slice(&order.app_data.to_be_bytes());
    order.fee_amount.to_big_endian(&mut result[74..106]);
    result[106] = encode_order_flags(order);
    executed_amount.to_big_endian(&mut result[107..139]);
    result[139..141].copy_from_slice(&fee_discount.to_be_bytes());
    result[141] = order.signature.v;
    result[142..174].copy_from_slice(order.signature.r.as_bytes());
    result[174..206].copy_from_slice(order.signature.s.as_bytes());
    result
}

fn encode_order_flags(order: &OrderCreation) -> u8 {
    let mut result = 0u8;
    if matches!(order.kind, OrderKind::Buy) {
        result |= 0b00000001;
    };
    if order.partially_fillable {
        result |= 0b00000010;
    }
    result
}

// None if length doesn't fit in 3 bytes.
pub fn encode_interaction_data_length(length: usize) -> Option<[u8; 3]> {
    let bytes = length.to_be_bytes();
    let (left, right) = bytes.split_at(bytes.len() - 3);
    if left.iter().any(|byte| *byte != 0) {
        return None;
    }
    // Need unwrap because technically `usize` could be smaller than 3 bytes in which case
    // converting `right` to [u8; 3] would fail.
    Some(right.try_into().unwrap())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use model::Signature;
    use primitive_types::{H160, H256};

    pub fn u8_as_32_bytes_be(u: u8) -> [u8; 32] {
        let mut result = [0u8; 32];
        result[31] = u;
        result
    }

    #[test]
    fn encode_trade_() {
        let order = OrderCreation {
            sell_token: H160::zero(),
            buy_token: H160::zero(),
            sell_amount: 4.into(),
            buy_amount: 5.into(),
            valid_to: 6,
            app_data: 7,
            fee_amount: 8.into(),
            kind: OrderKind::Buy,
            partially_fillable: true,
            signature: Signature {
                v: 9,
                r: H256::from_low_u64_be(10),
                s: H256::from_low_u64_be(11),
            },
        };
        let encoded = encode_trade(&order, 1, 2, &3.into(), 4);
        assert_eq!(encoded[0], 1);
        assert_eq!(encoded[1], 2);
        assert_eq!(encoded[2..34], u8_as_32_bytes_be(4));
        assert_eq!(encoded[34..66], u8_as_32_bytes_be(5));
        assert_eq!(encoded[66..70], [0, 0, 0, 6]);
        assert_eq!(encoded[70..74], [0, 0, 0, 7]);
        assert_eq!(encoded[74..106], u8_as_32_bytes_be(8));
        assert_eq!(encoded[106], 0b11);
        assert_eq!(encoded[107..139], u8_as_32_bytes_be(3));
        assert_eq!(encoded[139..141], [0, 4]);
        assert_eq!(encoded[141], 9);
        assert_eq!(encoded[142..174], u8_as_32_bytes_be(10));
        assert_eq!(encoded[174..206], u8_as_32_bytes_be(11));
    }
}

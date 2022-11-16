use database::{ethflow_orders::EthOrderPlacement, orders::Order};
use ethcontract::{Bytes, H160, U256};
use number_conversions::big_decimal_to_u256;
// Data structure reflecting the contract ethflow order
// https://github.com/cowprotocol/ethflowcontract/blob/main/src/libraries/EthFlowOrder.sol#L19
pub struct EthflowOrder {
    pub buy_token: H160,
    pub receiver: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub app_data: Bytes<[u8; 32]>,
    pub fee_amount: U256,
    pub valid_to: u32,
    pub partially_fillable: bool,
    pub quote_id: i64,
}

impl EthflowOrder {
    pub fn encode(&self) -> EncodedEthflowOrder {
        (
            self.buy_token,
            self.receiver,
            self.sell_amount,
            self.buy_amount,
            self.app_data,
            self.fee_amount,
            self.valid_to,
            self.partially_fillable,
            self.quote_id,
        )
    }
}

pub type EncodedEthflowOrder = (
    H160,            // buyToken
    H160,            // receiver
    U256,            // sellAmount
    U256,            // buyAmount
    Bytes<[u8; 32]>, // appData
    U256,            // feeAmount
    u32,             // validTo
    bool,            // flags
    i64,             // quoteId
);

pub fn order_to_ethflow_data(
    order: Order,
    ethflow_order_placement: EthOrderPlacement,
) -> EthflowOrder {
    EthflowOrder {
        buy_token: H160(order.buy_token.0),
        receiver: H160(order.receiver.unwrap().0), // ethflow orders have always a
        // receiver. It's enforced by the contract.
        sell_amount: big_decimal_to_u256(&order.sell_amount).unwrap(),
        buy_amount: big_decimal_to_u256(&order.buy_amount).unwrap(),
        app_data: Bytes(order.app_data.0),
        fee_amount: big_decimal_to_u256(&order.fee_amount).unwrap(),
        valid_to: ethflow_order_placement.valid_to as u32,
        partially_fillable: order.partially_fillable,
        quote_id: 0i64, // quoteId is not important for refunding and will be ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::byte_array::ByteArray;
    use number_conversions::u256_to_big_decimal;

    #[test]
    fn test_order_to_ethflow_data() {
        let buy_token = ByteArray([1u8; 20]);
        let receiver = ByteArray([3u8; 20]);
        let sell_amount = U256::from_dec_str("1").unwrap();
        let buy_amount = U256::from_dec_str("2").unwrap();
        let app_data = ByteArray([3u8; 32]);
        let fee_amount = U256::from_dec_str("3").unwrap();
        let valid_to = 234u32;

        let order = Order {
            buy_token,
            receiver: Some(receiver),
            sell_amount: u256_to_big_decimal(&sell_amount),
            buy_amount: u256_to_big_decimal(&buy_amount),
            valid_to: valid_to.into(),
            app_data,
            fee_amount: u256_to_big_decimal(&fee_amount),
            ..Default::default()
        };
        let ethflow_order = EthOrderPlacement {
            valid_to: valid_to.into(),
            ..Default::default()
        };
        let expected_encoded_order = (
            H160(order.buy_token.0),
            H160(order.receiver.unwrap().0),
            big_decimal_to_u256(&order.sell_amount).unwrap(),
            big_decimal_to_u256(&order.buy_amount).unwrap(),
            Bytes(order.app_data.0),
            big_decimal_to_u256(&order.fee_amount).unwrap(),
            ethflow_order.valid_to as u32,
            order.partially_fillable,
            0i64,
        );
        assert_eq!(
            order_to_ethflow_data(order, ethflow_order).encode(),
            expected_encoded_order
        );
    }
}

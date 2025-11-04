use {
    contracts::alloy::CoWSwapEthFlow,
    database::{ethflow_orders::EthOrderData, orders::Order},
    ethcontract::{Bytes, H160, U256},
    ethrpc::alloy::conversions::IntoAlloy,
    number::conversions::big_decimal_to_u256,
};

// Data structure reflecting the contract ethflow order
// https://github.com/cowprotocol/ethflowcontract/blob/main/src/libraries/EthFlowOrder.sol#L19
#[derive(Clone)]
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

impl From<EthflowOrder> for CoWSwapEthFlow::EthFlowOrder::Data {
    fn from(value: EthflowOrder) -> Self {
        CoWSwapEthFlow::EthFlowOrder::Data {
            buyToken: value.buy_token.into_alloy(),
            receiver: value.receiver.into_alloy(),
            sellAmount: value.sell_amount.into_alloy(),
            buyAmount: value.buy_amount.into_alloy(),
            appData: value.app_data.0.into(),
            feeAmount: value.fee_amount.into_alloy(),
            validTo: value.valid_to,
            partiallyFillable: value.partially_fillable,
            quoteId: value.quote_id,
        }
    }
}

pub fn order_to_ethflow_data(order: Order, ethflow_order_placement: EthOrderData) -> EthflowOrder {
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

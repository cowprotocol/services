use crate::{deploy::Contracts, tx_value};
use contracts::{CoWSwapEthFlow, WETH9};
use ethcontract::{transaction::TransactionResult, Account, Bytes, H160, H256};
use hex_literal::hex;
use model::{
    order::{Order, OrderBuilder, OrderClass, OrderKind},
    quote::OrderQuoteResponse,
    signature::hashed_eip712_message,
    DomainSeparator,
};
use refunder::{
    ethflow_order::EthflowOrder,
    refund_service::{INVALIDATED_OWNER, NO_OWNER},
};

pub struct ExtendedEthFlowOrder(pub EthflowOrder);

impl ExtendedEthFlowOrder {
    pub fn from_quote(quote_response: &OrderQuoteResponse, valid_to: u32) -> Self {
        let quote = &quote_response.quote;
        ExtendedEthFlowOrder(EthflowOrder {
            buy_token: quote.buy_token,
            receiver: quote.receiver.expect("eth-flow order without receiver"),
            sell_amount: quote.sell_amount,
            buy_amount: quote.buy_amount,
            app_data: ethcontract::Bytes(quote.app_data.0),
            fee_amount: quote.fee_amount,
            valid_to, // note: valid to in the quote is always unlimited
            partially_fillable: quote.partially_fillable,
            quote_id: quote_response.id.expect("No quote id"),
        })
    }

    fn to_cow_swap_order(&self, ethflow: &CoWSwapEthFlow, weth: &WETH9) -> Order {
        // Each ethflow user order has an order that is representing
        // it as EIP1271 order with a different owner and valid_to
        OrderBuilder::default()
            .with_kind(OrderKind::Sell)
            .with_sell_token(weth.address())
            .with_sell_amount(self.0.sell_amount)
            .with_fee_amount(self.0.fee_amount)
            .with_receiver(Some(self.0.receiver))
            .with_buy_token(self.0.buy_token)
            .with_buy_amount(self.0.buy_amount)
            .with_valid_to(u32::MAX)
            .with_app_data(self.0.app_data.0)
            .with_class(OrderClass::Market) // Eth-flow orders only support market orders at this point in time
            .with_eip1271(ethflow.address(), hex!("").into())
            .build()
    }

    pub fn include_slippage_bps(&self, slippage: u16) -> Self {
        let max_base_point = 10000;
        if slippage > max_base_point {
            panic!("Slippage must be specified in base points");
        }
        ExtendedEthFlowOrder(EthflowOrder {
            buy_amount: self.0.buy_amount * (max_base_point - slippage) / max_base_point,
            ..self.0
        })
    }

    pub async fn status(&self, contracts: &Contracts) -> EthFlowOrderOnchainStatus {
        contracts
            .ethflow
            .orders(Bytes(self.hash(contracts).await.0))
            .call()
            .await
            .expect("Couldn't fetch order status")
            .into()
    }

    pub async fn mine_order_creation(
        &self,
        owner: &Account,
        ethflow: &CoWSwapEthFlow,
    ) -> TransactionResult {
        tx_value!(
            owner,
            self.0.sell_amount + self.0.fee_amount,
            ethflow.create_order(self.0.encode())
        )
    }

    async fn hash(&self, contracts: &Contracts) -> H256 {
        let domain_separator = DomainSeparator(
            contracts
                .gp_settlement
                .domain_separator()
                .call()
                .await
                .expect("Couldn't query domain separator")
                .0,
        );
        H256(hashed_eip712_message(
            &domain_separator,
            &self
                .to_cow_swap_order(&contracts.ethflow, &contracts.weth)
                .data
                .hash_struct(),
        ))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EthFlowOrderOnchainStatus {
    Invalidated,
    Created(H160, u32),
    Free,
}

impl From<(H160, u32)> for EthFlowOrderOnchainStatus {
    fn from((owner, valid_to): (H160, u32)) -> Self {
        match owner {
            owner if owner == NO_OWNER => Self::Free,
            owner if owner == INVALIDATED_OWNER => Self::Invalidated,
            _ => Self::Created(owner, valid_to),
        }
    }
}

use crate::database::events::meta_to_event_index;
use anyhow::{anyhow, Context, Result};
use contracts::cowswap_onchain_orders::{
    event_data::OrderPlacement as ContractOrderPlacement, Event as ContractEvent,
};
use database::{
    ethflow_orders::EthOrderPlacement, events::EventIndex,
    onchain_broadcasted_orders::OnchainOrderPlacement, orders::Order, PgTransaction,
};
use ethcontract::Event as EthContractEvent;
use std::{collections::HashMap, convert::TryInto};

use super::{OnchainOrderCustomData, OnchainOrderParsing};

pub struct EthFlowOnchainOrderParser;

#[derive(Copy, Debug, Clone)]
pub struct EthFlowData {
    user_valid_to: u32,
}

#[async_trait::async_trait]
impl OnchainOrderParsing<EthFlowData, EthOrderPlacement> for EthFlowOnchainOrderParser {
    fn parse_custom_event_data(
        &self,
        contract_events: &[EthContractEvent<ContractEvent>],
    ) -> Result<Vec<(EventIndex, OnchainOrderCustomData<EthFlowData>)>> {
        contract_events
            .iter()
            .filter_map(|EthContractEvent { data, meta }| {
                let meta = match meta {
                    Some(meta) => meta,
                    None => return Some(Err(anyhow!("event without metadata"))),
                };
                let ContractEvent::OrderPlacement(event) = data;
                match convert_to_quote_id_and_user_valid_to(event) {
                    Ok((quote_id, user_valid_to)) => Some(Ok((
                        meta_to_event_index(meta),
                        OnchainOrderCustomData {
                            quote_id,
                            additional_data: Some(EthFlowData { user_valid_to }),
                        },
                    ))),
                    Err(err) => {
                        tracing::debug!(
                            "Error while converting quote id and user valid to: {:?}",
                            err
                        );
                        None
                    }
                }
            })
            .collect::<Result<Vec<_>>>()
    }

    async fn append_custom_order_info_to_db<'a>(
        &self,
        ex: &mut PgTransaction<'a>,
        custom_onchain_data: Vec<EthOrderPlacement>,
    ) -> Result<()> {
        database::ethflow_orders::append(ex, custom_onchain_data.as_slice())
            .await
            .context("append_ethflow_orders failed")
    }

    fn customized_event_data_for_event_index(
        &self,
        event_index: &EventIndex,
        order: &Order,
        hashmap: &HashMap<EventIndex, EthFlowData>,
        _onchain_order_placement: &OnchainOrderPlacement,
    ) -> EthOrderPlacement {
        EthOrderPlacement {
            uid: order.uid,
            // unwrap is allowed, as any missing event_index would have been filtered beforehand
            // by the implementation of the function parse_custom_event_data
            valid_to: hashmap.get(event_index).unwrap().user_valid_to as i64,
            is_refunded: false,
        }
    }
}

fn convert_to_quote_id_and_user_valid_to(
    order_placement: &ContractOrderPlacement,
) -> Result<(i64, u32)> {
    let data = order_placement.data.0.as_slice();
    anyhow::ensure!(data.len() == 12, "invalid data length");
    let quote_id = i64::from_be_bytes(data[0..8].try_into().unwrap());
    let user_valid_to = u32::from_be_bytes(data[8..12].try_into().unwrap());
    Ok((quote_id, user_valid_to))
}

#[cfg(test)]
mod test {
    use ethcontract::{Bytes, EventMetadata, H160, U256};
    use model::order::{OrderData, OrderKind};

    use super::*;

    #[test]
    pub fn test_convert_to_quote_id_and_user_valid_to() {
        let event_data = ContractOrderPlacement {
            data: ethcontract::Bytes(vec![
                0u8, 0u8, 3u8, 2u8, 0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8,
            ]),
            ..Default::default()
        };
        let expected_user_valid_to = 0x00_00_01_02;
        let expected_quote_id = 0x00_00_03_02_00_00_01_02;
        let result = convert_to_quote_id_and_user_valid_to(&event_data).unwrap();
        assert_eq!(result.1, expected_user_valid_to);
        assert_eq!(result.0, expected_quote_id);
    }

    #[test]
    pub fn parse_custom_event_data_filters_out_invalid_events() {
        let sell_token = H160::from([1; 20]);
        let buy_token = H160::from([2; 20]);
        let receiver = H160::from([3; 20]);
        let sender = H160::from([4; 20]);
        let sell_amount = U256::from_dec_str("10").unwrap();
        let buy_amount = U256::from_dec_str("11").unwrap();
        let valid_to = 1u32;
        let app_data = ethcontract::tokens::Bytes([5u8; 32]);
        let fee_amount = U256::from_dec_str("12").unwrap();
        let owner = H160::from([6; 20]);
        let order_placement = ContractOrderPlacement {
            sender,
            order: (
                sell_token,
                buy_token,
                receiver,
                sell_amount,
                buy_amount,
                valid_to,
                app_data,
                fee_amount,
                Bytes(OrderKind::SELL),
                true,
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
            ),
            signature: (0u8, Bytes(owner.as_ref().into())),
            data: ethcontract::Bytes(vec![
                0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8,
            ]),
        };
        let event_data = EthContractEvent {
            data: ContractEvent::OrderPlacement(order_placement.clone()),
            meta: Some(EventMetadata {
                block_number: 1,
                log_index: 0usize,
                ..Default::default()
            }),
        };
        let ethflow_onchain_order_parser = EthFlowOnchainOrderParser {};
        let result = ethflow_onchain_order_parser
            .parse_custom_event_data(vec![event_data].as_slice())
            .unwrap();
        assert_eq!(result.len(), 1);

        let mut order_placement_2 = order_placement;
        order_placement_2.data = Bytes(Vec::new()); // <- This will produce an error
        let event_data = EthContractEvent {
            data: ContractEvent::OrderPlacement(order_placement_2),
            meta: Some(EventMetadata {
                block_number: 1,
                log_index: 0usize,
                ..Default::default()
            }),
        };
        let result = ethflow_onchain_order_parser
            .parse_custom_event_data(vec![event_data].as_slice())
            .unwrap();
        assert_eq!(result.len(), 0);
    }
}

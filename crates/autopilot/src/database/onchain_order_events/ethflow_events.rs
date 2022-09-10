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
use unzip_n::unzip_n;

use super::{CustomOnchainOrderParsing, CustomParsedOnchaninData};

unzip_n!(pub 3);

pub struct EthFlowOnchainOrderParser {}

#[async_trait::async_trait]
impl<'a> CustomOnchainOrderParsing<'a, u32, EthOrderPlacement> for EthFlowOnchainOrderParser {
    fn parse_custom_event_data(
        &self,
        contract_events: &[EthContractEvent<ContractEvent>],
    ) -> Result<Vec<(EventIndex, CustomParsedOnchaninData<u32>)>> {
        contract_events
            .iter()
            .filter_map(|EthContractEvent { data, meta }| {
                let meta = match meta {
                    Some(meta) => meta,
                    None => return Some(Err(anyhow!("event without metadata"))),
                };
                let ContractEvent::OrderPlacement(event) = data;
                let quote_and_valid_to_touple = convert_to_quote_id_and_user_valid_to(event);
                match quote_and_valid_to_touple {
                    Ok(touple) => Some(Ok((
                        meta_to_event_index(meta),
                        CustomParsedOnchaninData {
                            quote_id: touple.0,
                            additional_data: Some(touple.1),
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

    async fn append_custom_order_info_to_db(
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
        hashmap: &HashMap<EventIndex, u32>,
        _onchain_order_placement: &OnchainOrderPlacement,
    ) -> EthOrderPlacement {
        EthOrderPlacement {
            uid: order.uid,
            // unwrap is allowed, as any missing event_index would have been filtered beforehand
            // by the implementation of the function parse_custom_event_data
            valid_to: *hashmap.get(event_index).unwrap() as i64,
        }
    }
}

fn convert_to_quote_id_and_user_valid_to(
    order_placement: &ContractOrderPlacement,
) -> Result<(i64, u32)> {
    let user_valid_to_bytes = TryInto::<[u8; 4]>::try_into(
        order_placement
            .data
            .0
            .iter()
            .take(4)
            .copied()
            .collect::<Vec<u8>>(),
    )
    .map_err(|err| anyhow!("Error while decoding user_valid_to data: {:?}", err))?;
    println!("{:?}", user_valid_to_bytes);
    let user_valid_to = u32::from_be_bytes(user_valid_to_bytes);
    let quote_id_bytes = TryInto::<[u8; 8]>::try_into(
        order_placement
            .data
            .0
            .iter()
            .skip(4)
            .take(8)
            .copied()
            .collect::<Vec<u8>>(),
    )
    .map_err(|err| anyhow!("Error while decoding user_valid_to data: {:?}", err))?;

    let quote_id = i64::from_be_bytes(quote_id_bytes);
    Ok((quote_id, user_valid_to))
}

#[cfg(test)]
mod test {
    use ethcontract::{Bytes, EventMetadata, H160, H256, U256};
    use model::order::OrderData;

    use super::*;

    #[test]
    pub fn test_convert_to_quote_id_and_user_valid_to() {
        let event_data = ContractOrderPlacement {
            data: ethcontract::Bytes(vec![
                0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8,
            ]),
            ..Default::default()
        };
        let expected_user_valid_to = 16u32.pow(2) + 2;
        let expected_quote_id = 16i64.pow(8) * (16i64.pow(2) + 2) + 16i64.pow(2) + 2;
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
                Bytes(OrderData::KIND_SELL),
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
                // todo: implement default for EvetMetadata
                address: H160::zero(),
                block_hash: H256::zero(),
                block_number: 1,
                transaction_hash: H256::zero(),
                transaction_index: 0usize,
                log_index: 0usize,
                transaction_log_index: None,
                log_type: None,
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
                // todo: implement default for EvetMetadata
                address: H160::zero(),
                block_hash: H256::zero(),
                block_number: 1,
                transaction_hash: H256::zero(),
                transaction_index: 0usize,
                log_index: 0usize,
                transaction_log_index: None,
                log_type: None,
            }),
        };
        let result = ethflow_onchain_order_parser
            .parse_custom_event_data(vec![event_data].as_slice())
            .unwrap();
        assert_eq!(result.len(), 0);
    }
}

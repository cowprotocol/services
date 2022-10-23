use crate::database::events::meta_to_event_index;
use anyhow::{anyhow, Context, Result};
use contracts::cowswap_onchain_orders::{
    event_data::OrderPlacement as ContractOrderPlacement, Event as ContractEvent,
};
use database::{
    byte_array::ByteArray,
    ethflow_orders::EthOrderPlacement,
    events::EventIndex,
    onchain_broadcasted_orders::OnchainOrderPlacement,
    orders::{Interaction, Order},
    PgTransaction,
};
use ethcontract::{dyns::DynWeb3, BlockId, BlockNumber, Event as EthContractEvent};
use hex_literal::hex;
use num::{bigint::ToBigInt, BigRational, ToPrimitive};
use shared::{conversions::U256Ext, Web3};
use sqlx::types::BigDecimal;
use std::{collections::HashMap, convert::TryInto};

use super::{OnchainOrderCustomData, OnchainOrderParsing};

#[derive(Debug, Clone)]
pub struct EthFlowOnchainOrderParser {
    web3: Web3,
}

impl EthFlowOnchainOrderParser {
    pub fn new(web3: DynWeb3) -> Self {
        EthFlowOnchainOrderParser { web3 }
    }

    async fn get_unix_timestamp_of_block(&self, block_number: i64) -> Result<i64> {
        self.web3
            .eth()
            .block(BlockId::Number(BlockNumber::Number(block_number.into())))
            .await
            .ok()
            .flatten()
            .map(|block| block.timestamp.as_u64() as i64)
            .context("could not find block's timestamp")
    }
}

// 4c84c1c8 is the identifier of the following function:
// https://github.com/cowprotocol/ethflowcontract/blob/main/src/CoWSwapEthFlow.sol#L57
const WRAP_ALL_SELECTOR: [u8; 4] = hex!("4c84c1c8");

#[derive(Copy, Debug, Clone)]
pub struct EthFlowData {
    user_valid_to: u32,
}

#[derive(Debug, Clone)]
pub struct EthFlowDataForDb {
    eth_order_placement: EthOrderPlacement,
    pre_interaction: Interaction,
}

#[async_trait::async_trait]
impl OnchainOrderParsing<EthFlowData, EthFlowDataForDb> for EthFlowOnchainOrderParser {
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
        custom_onchain_data: Vec<EthFlowDataForDb>,
    ) -> Result<()> {
        let (eth_order_placements, pre_interactions_data): (
            Vec<EthOrderPlacement>,
            Vec<(database::OrderUid, Interaction)>,
        ) = custom_onchain_data
            .iter()
            .map(|data| {
                (
                    data.eth_order_placement.clone(),
                    (data.eth_order_placement.uid, data.pre_interaction.clone()),
                )
            })
            .unzip();
        database::ethflow_orders::append(ex, eth_order_placements.as_slice())
            .await
            .context("append_ethflow_orders failed during appending eth order placement data")?;
        database::orders::insert_pre_interactions(ex, pre_interactions_data.as_slice())
            .await
            .context("append_ethflow_orders failed during appending pre_interactions")
    }

    async fn customized_event_data_for_event_index(
        &self,
        event_index: &EventIndex,
        quote: &shared::order_quoting::Quote,
        order: &Order,
        hashmap: &HashMap<EventIndex, EthFlowData>,
        _onchain_order_placement: &OnchainOrderPlacement,
    ) -> Result<EthFlowDataForDb> {
        let slippage = BigRational::new(
            quote.data.quoted_buy_amount.to_big_int(),
            order.buy_amount.to_bigint().unwrap(),
        )
        .to_f64()
        .unwrap_or(f64::NAN);
        let unix_timestamp_of_block = self
            .get_unix_timestamp_of_block(event_index.block_number)
            .await?;
        // unwrap is allowed, as any missing event_index would have been
        // filtered beforehand by the implementation of the function
        // parse_custom_event_data
        let valid_to = hashmap.get(event_index).unwrap().user_valid_to as i64;
        let validity_duration = valid_to - unix_timestamp_of_block;
        Ok(EthFlowDataForDb {
            eth_order_placement: EthOrderPlacement {
                uid: order.uid,
                valid_to,
                is_refunded: false,
                validity_duration,
                slippage,
            },
            // The following interaction calls the wrap_all() function on the
            // ethflow contract in order to wrap all existing ether to weth,
            // such that the eth can be used as WETH by the cow protocol
            pre_interaction: Interaction {
                // For ethflow orders, the owner is always the ethflow contract
                target: ByteArray(order.owner.0),
                value: BigDecimal::new(0.into(), 1),
                data: WRAP_ALL_SELECTOR.to_vec(),
            },
        })
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

    use ethcontract_mock::Mock;
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
        let ethflow_onchain_order_parser = EthFlowOnchainOrderParser::new(Mock::new(1).web3());
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

use {
    super::{OnchainOrderCustomData, OnchainOrderParsing},
    crate::database::events::meta_to_event_index,
    anyhow::{Context, Result, anyhow},
    contracts::{
        GPv2Settlement,
        cowswap_onchain_orders::{
            Event as ContractEvent,
            event_data::OrderPlacement as ContractOrderPlacement,
        },
        deployment_block,
    },
    database::{
        PgTransaction,
        byte_array::ByteArray,
        ethflow_orders::EthOrderPlacement,
        events::EventIndex,
        onchain_broadcasted_orders::OnchainOrderPlacement,
        orders::{ExecutionTime, Interaction, Order},
    },
    ethcontract::Event as EthContractEvent,
    ethrpc::{
        Web3,
        block_stream::{BlockNumberHash, block_number_to_block_number_hash},
    },
    hex_literal::hex,
    sqlx::{PgPool, types::BigDecimal},
    std::{collections::HashMap, convert::TryInto},
    tracing::instrument,
    web3::types::U64,
};

// 4c84c1c8 is the identifier of the following function:
// https://github.com/cowprotocol/ethflowcontract/blob/main/src/CoWSwapEthFlow.sol#L57
pub const WRAP_ALL_SELECTOR: [u8; 4] = hex!("4c84c1c8");

pub struct EthFlowOnchainOrderParser;

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
                let event = match data {
                    ContractEvent::OrderPlacement(event) => event,
                    _ => return None,
                };
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
        database::ethflow_orders::insert_or_overwrite_orders(ex, eth_order_placements.as_slice())
            .await
            .context("append_ethflow_orders failed during appending eth order placement data")?;
        database::orders::insert_or_overwrite_interactions(ex, pre_interactions_data.as_slice())
            .await
            .context("append_ethflow_orders failed during appending pre_interactions")
    }

    fn customized_event_data_for_event_index(
        &self,
        event_index: &EventIndex,
        order: &Order,
        hashmap: &HashMap<EventIndex, EthFlowData>,
        _onchain_order_placement: &OnchainOrderPlacement,
    ) -> EthFlowDataForDb {
        EthFlowDataForDb {
            eth_order_placement: EthOrderPlacement {
                uid: order.uid,
                // unwrap is allowed, as any missing event_index would have been filtered beforehand
                // by the implementation of the function parse_custom_event_data
                valid_to: hashmap.get(event_index).unwrap().user_valid_to as i64,
            },
            // The following interaction calls the wrap_all() function on the ethflow contract
            // in order to wrap all existing ether to weth, such that the eth can be used as
            // WETH by the cow protocol
            pre_interaction: Interaction {
                // For ethflow orders, the owner is always the ethflow contract
                target: ByteArray(order.owner.0),
                value: BigDecimal::new(0.into(), 1),
                data: WRAP_ALL_SELECTOR.to_vec(),
                index: 0,
                execution: ExecutionTime::Pre,
            },
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

async fn settlement_deployment_block_number_hash(
    web3: &Web3,
    chain_id: u64,
) -> Result<BlockNumberHash> {
    let block_number = deployment_block(GPv2Settlement::raw_contract(), chain_id)?;
    block_number_to_block_number_hash(web3, U64::from(block_number).into())
        .await
        .context("Deployment block not found")
}

/// The block from which to start indexing eth-flow events. Note that this
/// function is expected to be used at the start of the services and will panic
/// if it cannot retrieve the information it needs.
#[instrument(skip_all)]
pub async fn determine_ethflow_indexing_start(
    skip_event_sync_start: &Option<BlockNumberHash>,
    ethflow_indexing_start: Option<u64>,
    web3: &Web3,
    chain_id: u64,
    db: &crate::database::Postgres,
) -> BlockNumberHash {
    if let Some(block_number_hash) = skip_event_sync_start {
        return *block_number_hash;
    }

    find_indexing_start_block(
        &db.pool,
        web3,
        crate::database::onchain_order_events::INDEX_NAME,
        ethflow_indexing_start,
        Some(chain_id),
    )
    .await
    .expect("Should be able to find last indexed onchain order block")
    .expect("No last indexed block found for ethflow orders")
}

/// Determines the starting block number and hash for indexing eth-flow refund
/// events.
///
/// This function computes the most appropriate starting block by evaluating
/// several potential sources:
/// 0. If `skip_event_sync_start` is provided, it uses this value directly and
///    returns early.
/// 1. If a corresponding record in the `last_indexed_blocks` exists, use this
///    value.
/// 2. Otherwise, use `ethflow_indexing_start` if it is provided.
/// 3. Fallback to the settlement deployment block number if no other options
///    are available.
/// 4. Finally, try fetching the block using the provided `web3` instance to
///    ensure the node is able to continue indexing.
///
/// # Panics
/// Note that this function is expected to be used at the start of the services
/// and will panic  if it cannot retrieve the information it needs.
#[instrument(skip_all)]
pub async fn determine_ethflow_refund_indexing_start(
    skip_event_sync_start: &Option<BlockNumberHash>,
    ethflow_indexing_start: Option<u64>,
    web3: &Web3,
    chain_id: u64,
    db: crate::database::Postgres,
) -> BlockNumberHash {
    if let Some(block_number_hash) = skip_event_sync_start {
        return *block_number_hash;
    }

    let ethflow_refund_indexing_start = find_indexing_start_block(
        &db.pool,
        web3,
        crate::database::ethflow_events::event_storing::INDEX_NAME,
        ethflow_indexing_start,
        None,
    )
    .await
    .expect("Should be able to find last indexed refund block");
    let settlement_contract_indexing_start = find_indexing_start_block(
        &db.pool,
        web3,
        crate::boundary::events::settlement::INDEX_NAME,
        None,
        Some(chain_id),
    )
    .await
    .expect("Should be able to find last indexed settlement block");

    vec![
        ethflow_refund_indexing_start,
        settlement_contract_indexing_start,
    ]
    .into_iter()
    .flatten()
    .max_by_key(|(block_number, _)| *block_number)
    .expect("Should be able to find a valid start block")
}

/// 1. Check the `last_indexed_blocks` table for the `index_name`.
/// 2. If no value found or the index is 0, use `fallback_start_block`, if
///    provided.
/// 3. Fallback to the settlement deployment block number, if the `chain_id` is
///    provided.
/// 4. Try to fetch the block number to ensure the node is able to continue
///    indexing.
async fn find_indexing_start_block(
    db: &PgPool,
    web3: &Web3,
    index_name: &str,
    fallback_start_block: Option<u64>,
    settlement_fallback_chain_id: Option<u64>,
) -> Result<Option<BlockNumberHash>> {
    let last_indexed_block = crate::boundary::events::read_last_block_from_db(db, index_name)
        .await
        .context("failed to read last indexed block from db")?;

    if last_indexed_block > 0 {
        return block_number_to_block_number_hash(web3, U64::from(last_indexed_block).into())
            .await
            .map(Some)
            .context("failed to fetch block");
    }
    if let Some(start_block) = fallback_start_block {
        return block_number_to_block_number_hash(web3, start_block.into())
            .await
            .map(Some)
            .context("failed to fetch fallback indexing start block");
    }
    if let Some(chain_id) = settlement_fallback_chain_id {
        return settlement_deployment_block_number_hash(web3, chain_id)
            .await
            .map(Some)
            .context("failed to fetch settlement deployment block");
    }

    Ok(None)
}

#[cfg(test)]
mod test {
    use {
        super::*,
        ethcontract::{Bytes, EventMetadata, H160, U256},
        model::order::{BuyTokenDestination, OrderKind, SellTokenSource},
    };

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
                Bytes(SellTokenSource::ERC20),
                Bytes(BuyTokenDestination::ERC20),
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

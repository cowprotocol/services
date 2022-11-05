use crate::{encoding::EncodedSettlement, settlement::Settlement};
use anyhow::{anyhow, Context, Error, Result};
use contracts::GPv2Settlement;
use ethcontract::{
    batch::CallBatch,
    contract::MethodBuilder,
    dyns::{DynMethodBuilder, DynTransport},
    errors::ExecutionError,
    transaction::TransactionBuilder,
    Account,
};
use futures::FutureExt;
use gas_estimation::GasPrice1559;
use primitive_types::{H160, H256, U256};
use shared::{
    tenderly_api::{SimulationRequest, TenderlyApi},
    Web3,
};
use web3::types::{AccessList, BlockId};

const SIMULATE_BATCH_SIZE: usize = 10;

/// The maximum amount the base gas fee can increase from one block to the other.
///
/// This is derived from [EIP-1559](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-1559.md):
/// ```text
/// BASE_FEE_MAX_CHANGE_DENOMINATOR = 8
/// base_fee_per_gas_delta = max(parent_base_fee_per_gas * gas_used_delta // parent_gas_target // BASE_FEE_MAX_CHANGE_DENOMINATOR, 1)
/// ```
///
/// Because the elasticity factor is 2, this means that the highes possible `gas_used_delta == parent_gas_target`.
/// Therefore, the highest possible `base_fee_per_gas_delta` is `parent_base_fee_per_gas / 8`.
///
/// Example of this in action:
/// [Block 12998225](https://etherscan.io/block/12998225) with base fee of `43.353224173` and ~100% over the gas target.
/// Next [block 12998226](https://etherscan.io/block/12998226) has base fee of `48.771904644` which is an increase of ~12.5%.
const MAX_BASE_GAS_FEE_INCREASE: f64 = 1.125;

pub async fn simulate_and_estimate_gas_at_current_block(
    settlements: impl Iterator<Item = (Account, Settlement, Option<AccessList>)>,
    contract: &GPv2Settlement,
    gas_price: GasPrice1559,
) -> Result<Vec<Result<U256, ExecutionError>>> {
    // Collect into Vec to not rely on Itertools::chunk which would make this future !Send.
    let settlements: Vec<_> = settlements.collect();

    // Force settlement simulations to be done in smaller batches. They can be
    // quite large and exert significant node pressure.
    let mut results = Vec::new();
    for chunk in settlements.chunks(SIMULATE_BATCH_SIZE) {
        let calls = chunk
            .iter()
            .map(|(account, settlement, access_list)| {
                let tx = settle_method(gas_price, contract, settlement.clone(), account.clone()).tx;
                let tx = match access_list {
                    Some(access_list) => tx.access_list(access_list.clone()),
                    None => tx,
                };
                tx.estimate_gas()
            })
            .collect::<Vec<_>>();
        let chuck_results = futures::future::join_all(calls).await;
        results.extend(chuck_results);
    }

    Ok(results)
}

#[allow(clippy::needless_collect)]
pub async fn simulate_and_error_with_tenderly_link(
    settlements: impl Iterator<Item = (Account, Settlement, Option<AccessList>)>,
    contract: &GPv2Settlement,
    web3: &Web3,
    gas_price: GasPrice1559,
    network_id: &str,
    block: u64,
    simulation_gas_limit: u128,
) -> Vec<Result<()>> {
    let mut batch = CallBatch::new(web3.transport());
    let futures = settlements
        .map(|(account, settlement, access_list)| {
            let method = settle_method(gas_price, contract, settlement, account);
            let method = match access_list {
                Some(access_list) => method.access_list(access_list),
                None => method,
            };
            let transaction_builder = method.tx.clone();
            let view = method
                .view()
                .block(BlockId::Number(block.into()))
                // Since we now supply the gas price for the simulation, make sure to also
                // set a gas limit so we don't get failed simulations because of insufficient
                // solver balance. The limit should be below the current block gas
                // limit of 30M gas
                .gas(simulation_gas_limit.into());
            (view.batch_call(&mut batch), transaction_builder)
        })
        .collect::<Vec<_>>();
    batch.execute_all(SIMULATE_BATCH_SIZE).await;

    futures
        .into_iter()
        .map(|(future, transaction_builder)| {
            future.now_or_never().unwrap().map(|_| ()).map_err(|err| {
                Error::new(err).context(tenderly_link(block, network_id, transaction_builder))
            })
        })
        .collect()
}

pub async fn simulate_before_after_access_list(
    web3: &Web3,
    tenderly: &dyn TenderlyApi,
    network_id: String,
    transaction_hash: H256,
) -> Result<f64> {
    let transaction = web3
        .eth()
        .transaction(transaction_hash.into())
        .await?
        .context("no transaction found")?;

    if transaction.access_list.is_none() {
        return Err(anyhow!(
            "no need to analyze access list since no access list was found in mined transaction"
        ));
    }

    let (block_number, from, to, transaction_index) = (
        transaction
            .block_number
            .context("no block number field exist")?
            .as_u64(),
        transaction.from.context("no from field exist")?,
        transaction.to.context("no to field exist")?,
        transaction
            .transaction_index
            .context("no transaction_index field exist")?
            .as_u64(),
    );

    let request = SimulationRequest {
        network_id,
        block_number: Some(block_number),
        transaction_index: Some(transaction_index),
        from,
        input: transaction.input.0,
        to,
        gas: Some(transaction.gas.as_u64()),
        ..Default::default()
    };

    let gas_used_without_access_list = tenderly.simulate(request).await?.transaction.gas_used;
    let gas_used_with_access_list = web3
        .eth()
        .transaction_receipt(transaction_hash)
        .await?
        .ok_or_else(|| anyhow!("no transaction receipt"))?
        .gas_used
        .ok_or_else(|| anyhow!("no gas used field"))?;

    Ok(gas_used_without_access_list as f64 - gas_used_with_access_list.to_f64_lossy())
}

pub fn settle_method(
    gas_price: GasPrice1559,
    contract: &GPv2Settlement,
    settlement: Settlement,
    account: Account,
) -> MethodBuilder<DynTransport, ()> {
    // Increase the gas price by the highest possible base gas fee increase. This
    // is done because the between retrieving the gas price and executing the simulation,
    // a block may have been mined that increases the base gas fee and causes the
    // `eth_call` simulation to fail with `max fee per gas less than block base fee`.
    let gas_price = gas_price.bump(MAX_BASE_GAS_FEE_INCREASE);
    settle_method_builder(contract, settlement.into(), account)
        .gas_price(crate::into_gas_price(&gas_price))
}

pub fn settle_method_builder(
    contract: &GPv2Settlement,
    settlement: EncodedSettlement,
    from: Account,
) -> DynMethodBuilder<()> {
    contract
        .settle(
            settlement.tokens,
            settlement.clearing_prices,
            settlement.trades,
            settlement.interactions,
        )
        .from(from)
}

/// The call data of a settle call with this settlement.
pub fn call_data(settlement: EncodedSettlement) -> Vec<u8> {
    let contract = GPv2Settlement::at(&shared::transport::dummy::web3(), H160::default());
    let method = contract.settle(
        settlement.tokens,
        settlement.clearing_prices,
        settlement.trades,
        settlement.interactions,
    );
    // Unwrap because there should always be calldata.
    method.tx.data.unwrap().0
}

// Creates a simulation link in the gp-v2 tenderly workspace
pub fn tenderly_link(
    current_block: u64,
    network_id: &str,
    tx: TransactionBuilder<DynTransport>,
) -> String {
    // Tenderly simulates transactions for block N at transaction index 0, while
    // `eth_call` simulates transactions "on top" of the block (i.e. after the
    // last transaction index). Therefore, in order for the Tenderly simulation
    // to be as close as possible to the `eth_call`, we want to create it on the
    // next block (since `block_N{tx_last} ~= block_(N+1){tx_0}`).
    let next_block = current_block + 1;
    format!(
        "https://dashboard.tenderly.co/gp-v2/staging/simulator/new?block={}&blockIndex=0&from={:#x}&gas=8000000&gasPrice=0&value=0&contractAddress={:#x}&network={}&rawFunctionInput=0x{}",
        next_block,
        tx.from.unwrap().address(),
        tx.to.unwrap(),
        network_id,
        hex::encode(tx.data.unwrap().0)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        interactions::allowances::{Allowances, MockAllowanceManaging},
        liquidity::{
            balancer_v2::SettlementHandler, order_converter::OrderConverter,
            slippage::SlippageContext, uniswap_v2::Inner, ConstantProductOrder, Liquidity,
            StablePoolOrder,
        },
        settlement::InternalizationStrategy,
        solver::http_solver::settlement::{convert_settlement, SettlementContext},
    };
    use contracts::{BalancerV2Vault, IUniswapLikeRouter, UniswapV2Router02, WETH9};
    use ethcontract::{Account, PrivateKey};
    use maplit::hashmap;
    use model::{order::Order, TokenPair};
    use num::{rational::Ratio, BigRational};
    use serde_json::json;
    use shared::{
        http_solver::model::SettledBatchAuctionModel,
        sources::balancer_v2::pools::{common::TokenState, stable::AmplificationParameter},
        tenderly_api::TenderlyHttpApi,
        transport::create_env_test_transport,
    };
    use std::{
        str::FromStr,
        sync::{Arc, Mutex},
    };

    // cargo test -p solver settlement_simulation::tests::mainnet -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        // Create some bogus settlements to see that the simulation returns an error.
        shared::tracing::initialize(
            "info,solver=debug,shared=debug,shared::transport=trace",
            tracing::Level::ERROR.into(),
        );
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let block = web3.eth().block_number().await.unwrap().as_u64();
        let network_id = web3.net().version().await.unwrap();
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let account = Account::Offline(PrivateKey::from_raw([1; 32]).unwrap(), None);

        let settlements = vec![
            (
                account.clone(),
                Settlement::with_trades(Default::default(), vec![Default::default()], vec![]),
                None,
            ),
            (account.clone(), Settlement::new(Default::default()), None),
        ];
        let result = simulate_and_error_with_tenderly_link(
            settlements.iter().cloned(),
            &contract,
            &web3,
            Default::default(),
            network_id.as_str(),
            block,
            15000000u128,
        )
        .await;
        let _ = dbg!(result);

        let result = simulate_and_estimate_gas_at_current_block(
            settlements.iter().cloned(),
            &contract,
            Default::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);

        let result = simulate_and_estimate_gas_at_current_block(
            std::iter::empty(),
            &contract,
            Default::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);
    }

    // cargo test decode_quasimodo_solution_with_liquidity_orders_and_simulate_onchain_tx -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn decode_quasimodo_solution_with_liquidity_orders_and_simulate_onchain_tx() {
        // This e2e test re-simulates the settlement from here: https://etherscan.io/tx/0x6756c294eb84c899247f2ec64d6eee73e7aaf50d6cb49ba9bab636f450240f51
        // This settlement was wrongly settled, because the liquidity order did receive a surplus.
        // The liquidity order is:
        // https://explorer.cow.fi/orders/0x4da985bb7639bdac928553d0c39a3840388e27f825c572bb8addb47ef2de1f03e63a13eedd01b624958acfe32145298788a7a7ba61be1542

        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let native_token_contract = WETH9::deployed(&web3)
            .await
            .expect("couldn't load deployed native token");
        let network_id = web3.net().version().await.unwrap();
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let uniswap_router = UniswapV2Router02::deployed(&web3).await.unwrap();
        let balancer_vault = BalancerV2Vault::deployed(&web3).await.unwrap();

        let account = Account::Local(
            "0xa6DDBD0dE6B310819b49f680F65871beE85f517e"
                .parse()
                .unwrap(),
            None,
        );
        let order_converter = OrderConverter {
            native_token: native_token_contract.clone(),
            fee_objective_scaling_factor: 0.91_f64,
        };
        let value = json!(
        {
            "creationDate": "2021-12-18T17:06:05.425889Z",
            "owner": "0x295a0bc540f3d9a9bd67a777ca9da9fb5619d3a9",
            "uid": "0x721f9c5c4bbadeff130c4b0279951a2703c91ccc440cd64acb6b11caba0c64e9295a0bc540f3d9a9bd67a777ca9da9fb5619d3a961be1c03",
            "availableBalance": "9437822596",
            "executedBuyAmount": "0",
            "executedSellAmount": "0",
            "executedSellAmountBeforeFees": "0",
            "executedFeeAmount": "0",
            "invalidated": false,
            "sellToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
            "buyToken": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "receiver": "0x295a0bc540f3d9a9bd67a777ca9da9fb5619d3a9",
            "sellAmount": "9413614019",
            "buyAmount": "2377438739352985079",
            "validTo": 1639848963u32,
            "appData": "0x487b02c558d729abaf3ecf17881a4181e5bc2446429a0995142297e897b6eb37",
            "feeAmount": "24208577",
            "fullFeeAmount": "49817596",
            "kind": "sell",
            "partiallyFillable": false,
            "signature": "0x50afa71e17cc7b1a7d5debf74b1baeebc9724539f92d056d58b0c1f95e19ef626583f3e9fe9ebc16324c28f88f80029ff04aa1d402972bfeacf93e052d3250ef1c",
            "signingScheme": "eip712",
            "status": "open",
            "settlementContract": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "isLiquidityOrder": false,
            "class": "ordinary",
        });
        let order0: Order = serde_json::from_value(value).unwrap();
        let value = json!(
        {
            "creationDate": "2021-12-24T05:02:18.624125Z",
            "owner": "0xe63a13eedd01b624958acfe32145298788a7a7ba",
            "uid": "0x4da985bb7639bdac928553d0c39a3840388e27f825c572bb8addb47ef2de1f03e63a13eedd01b624958acfe32145298788a7a7ba61be1542",
            "availableBalance": "106526950853",
            "executedBuyAmount": "0",
            "executedSellAmount": "0",
            "executedSellAmountBeforeFees": "0",
            "executedFeeAmount": "0",
            "invalidated": false,
            "sellToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
            "buyToken": "0xdac17f958d2ee523a2206206994597c13d831ec7",
            "receiver": "0xe63a13eedd01b624958acfe32145298788a7a7ba",
            "sellAmount": "11722136152",
            "buyAmount": "11727881818",
            "validTo": 1639847234u32,
            "appData": "0x00000000000000000000000055662e225a3376759c24331a9aed764f8f0c9fbb",
            "feeAmount": "3400559",
            "fullFeeAmount": "49915997",
            "kind": "buy",
            "partiallyFillable": false,
            "signature": "0x0701b6c9c5314b4d446229ba2940b6f2ad7600ff6579a77627d50307528c2f2d53fb9c889ba3f566eddb34b42e3ad0a2604b0b497b33af5c24927773912d05601c",
            "signingScheme": "ethsign",
            "status": "open",
            "settlementContract": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "isLiquidityOrder": true,
            "class": "liquidity",
        });
        let order1: Order = serde_json::from_value(value).unwrap();
        let value = json!(
        {
            "creationDate": "2021-12-18T16:46:41.271735Z",
            "owner": "0xf105e7d4dc8b1592e806a36c3b351a8b63b5c07c",
            "uid": "0x9a6986670e989c0bd4049d983ad574c2a8e8bdd6dd91473e197c2539caf8e025f105e7d4dc8b1592e806a36c3b351a8b63b5c07c61be1776",
            "availableBalance": "380000000000000000",
            "executedBuyAmount": "0",
            "executedSellAmount": "0",
            "executedSellAmountBeforeFees": "0",
            "executedFeeAmount": "0",
            "invalidated": false,
            "sellToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "buyToken": "0xdac17f958d2ee523a2206206994597c13d831ec7",
            "receiver": "0xf105e7d4dc8b1592e806a36c3b351a8b63b5c07c",
            "sellAmount": "372267382796377048",
            "buyAmount": "1475587283",
            "validTo": 1639847798u32,
            "appData": "0x487b02c558d729abaf3ecf17881a4181e5bc2446429a0995142297e897b6eb37",
            "feeAmount": "7732617203622952",
            "fullFeeAmount": "14232617203622952",
            "kind": "sell",
            "partiallyFillable": false,
            "signature": "0xaae201933a47b6e9d88ddeded8a64b11a4bcaa4e307263447af860a79301930d6c25b16e6ed15fa2e5a233be1e71e60acdd7fec6a52861dcf21d9c4720e1a2c01b",
            "signingScheme": "eip712",
            "status": "open",
            "settlementContract": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "isLiquidityOrder": false,
            "class": "ordinary",
        });
        let order2: Order = serde_json::from_value(value).unwrap();

        let orders = vec![order0, order1, order2];
        let orders = orders
            .into_iter()
            .map(|order| order_converter.normalize_limit_order(order).unwrap())
            .collect::<Vec<_>>();

        let cpo_0 = ConstantProductOrder {
            address: H160::from_low_u64_be(1),
            tokens: TokenPair::new(
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    .parse()
                    .unwrap(),
                "0xdac17f958d2ee523a2206206994597c13d831ec7"
                    .parse()
                    .unwrap(),
            )
            .unwrap(),
            reserves: (12779103255415685792803, 50331174049111),
            fee: Ratio::new(3, 1000),
            settlement_handling: Arc::new(Inner::new(
                IUniswapLikeRouter::at(&web3, uniswap_router.address()),
                contract.clone(),
                Mutex::new(Allowances::new(
                    contract.address(),
                    hashmap! {uniswap_router.address() => U256::from_dec_str("18000000000000000000000000").unwrap()},
                )),
            )),
        };

        let spo = StablePoolOrder {
            address: H160::from_low_u64_be(1),
            reserves: hashmap! {
                "0x6b175474e89094c44da98b954eedeac495271d0f".parse().unwrap() => TokenState {
                    balance: U256::from(46543572661097157184873466u128),
                    scaling_exponent: 18
                },
                "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".parse().unwrap() => TokenState {
                    balance: U256::from(50716887827666u128),
                    scaling_exponent: 6
                },
                "0xdac17f958d2ee523a2206206994597c13d831ec7".parse().unwrap() => TokenState{
                    balance: U256::from(38436050628181u128),
                                        scaling_exponent: 6
                                    },
            },
            fee: BigRational::new(1.into(), 1000.into()),
            amplification_parameter: AmplificationParameter::new(1573.into(), 1.into()).unwrap(),
            settlement_handling: Arc::new(SettlementHandler::new(
                "0x06df3b2bbb68adc8b0e302443692037ed9f91b42000000000000000000000063"
                    .parse()
                    .unwrap(),
                contract.clone(),
                balancer_vault,
                Arc::new(Allowances::new(
                    contract.address(),
                    hashmap! {"0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".parse().unwrap()=> U256::from_dec_str("18000000000000000000000000").unwrap()},
                )),
            )),
        };

        let liquidity = vec![
            Liquidity::ConstantProduct(cpo_0),
            Liquidity::BalancerStable(spo),
        ];
        let settlement_context = SettlementContext { orders, liquidity };
        let quasimodo_response = r#" {
            "amms": {
                "1": {
                    "amplification_parameter": "1573.000000",
                    "cost": {
                        "amount": "7174016181720000",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "execution": [
                        {
                            "buy_token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                            "exec_buy_amount": "21135750171",
                            "exec_plan": {
                                "position": 0,
                                "sequence": 0
                            },
                            "exec_sell_amount": "21129728791",
                            "sell_token": "0xdac17f958d2ee523a2206206994597c13d831ec7"
                        }
                    ],
                    "fee": "0.000100",
                    "kind": "Stable",
                    "mandatory": false,
                    "reserves": {
                        "0x6b175474e89094c44da98b954eedeac495271d0f": "46543572661097157184873466",
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "50716887827666",
                        "0xdac17f958d2ee523a2206206994597c13d831ec7": "38436050628181"
                    }
                },
                "0": {
                    "cost": {
                        "amount": "5661255302867976",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "execution": [
                        {
                            "buy_token": "0xdac17f958d2ee523a2206206994597c13d831ec7",
                            "exec_buy_amount": "7922480269",
                            "exec_plan": {
                                "position": 1,
                                "sequence": 0
                            },
                            "exec_sell_amount": "2005171356556612050",
                            "sell_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                        }
                    ],
                    "fee": "0.003000",
                    "kind": "ConstantProduct",
                    "mandatory": false,
                    "reserves": {
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1.277910e+22",
                        "0xdac17f958d2ee523a2206206994597c13d831ec7": "5.033117e+13"
                    }
                }
            },
            "metadata": {
                "has_solution": true,
                "result": "Optimal",
                "total_runtime": 0.247607875
            },
            "orders": {
                "0": {
                    "allow_partial_fill": false,
                    "buy_amount": "2377438739352985079",
                    "buy_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "cost": {
                        "amount": "3964540692423015",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "exec_buy_amount": "2377438739352985079",
                    "exec_sell_amount": "9413614019",
                    "fee": {
                        "amount": "49817596",
                        "token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                    },
                    "is_liquidity_order": false,
                    "is_sell_order": true,
                    "mandatory": false,
                    "sell_amount": "9413614019",
                    "sell_token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                },
                "1": {
                    "allow_partial_fill": false,
                    "buy_amount": "11727881818",
                    "buy_token": "0xdac17f958d2ee523a2206206994597c13d831ec7",
                    "cost": {
                        "amount": "3964540692423015",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "exec_buy_amount": "11727881818",
                    "exec_sell_amount": "11722136152",
                    "fee": {
                        "amount": "49915997",
                        "token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                    },
                    "is_liquidity_order": true,
                    "is_sell_order": false,
                    "mandatory": false,
                    "sell_amount": "11722136152",
                    "sell_token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                },
                "2": {
                    "allow_partial_fill": false,
                    "buy_amount": "1475587283",
                    "buy_token": "0xdac17f958d2ee523a2206206994597c13d831ec7",
                    "cost": {
                        "amount": "3964540692423015",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "exec_buy_amount": "1479366704",
                    "exec_sell_amount": "372267382796377048",
                    "fee": {
                        "amount": "14232617203622952",
                        "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                    },
                    "is_liquidity_order": false,
                    "is_sell_order": true,
                    "mandatory": false,
                    "sell_amount": "372267382796377048",
                    "sell_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                }
            },
            "prices": {
                "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "252553241991277046325219637",
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1000000000000000000",
                "0xdac17f958d2ee523a2206206994597c13d831ec7": "251639692692125716448920105"
            },
            "ref_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "tokens": {
                "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": {
                    "alias": "USDC",
                    "decimals": 6
                },
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                    "alias": "WETH",
                    "decimals": 18
                },
                "0xdac17f958d2ee523a2206206994597c13d831ec7": {
                    "alias": "USDT",
                    "decimals": 6
                }
            }
        }
        "#;
        let parsed_response = serde_json::from_str::<SettledBatchAuctionModel>(quasimodo_response);

        let settlements = convert_settlement(
            parsed_response.unwrap(),
            settlement_context,
            Arc::new(MockAllowanceManaging::new()),
            Arc::new(OrderConverter::test(H160([0x42; 20]))),
            SlippageContext::default(),
        )
        .await
        .map(|settlement| vec![settlement])
        .unwrap();
        let settlement = settlements.get(0).unwrap();
        let settlement_encoded = settlement
            .encoder
            .clone()
            .finish(InternalizationStrategy::SkipInternalizableInteraction);
        println!("Settlement_encoded: {:?}", settlement_encoded);
        let settlement = settle_method_builder(&contract, settlement_encoded, account).tx;
        println!(
            "Tenderly simulation for generated tx: {:?}",
            tenderly_link(13830346u64, &network_id, settlement)
        );
    }

    // cargo test -p solver settlement_simulation::tests::mainnet_chunked -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet_chunked() {
        shared::tracing::initialize(
            "info,solver=debug,shared=debug,shared::transport=trace",
            tracing::Level::ERROR.into(),
        );
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let account = Account::Offline(PrivateKey::from_raw([1; 32]).unwrap(), None);

        // 12 so that we hit more than one chunk.
        let settlements = vec![
            (account.clone(), Settlement::new(Default::default()), None);
            SIMULATE_BATCH_SIZE + 2
        ];
        let result = simulate_and_estimate_gas_at_current_block(
            settlements.iter().cloned(),
            &contract,
            GasPrice1559::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);
    }

    #[tokio::test]
    #[ignore]
    async fn simulate_before_after_access_list_test() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let transaction_hash =
            H256::from_str("e337fcd52afd6b98847baab279cda6c3980fcb185da9e959fd489ffd210eac60")
                .unwrap();
        let tenderly_api = TenderlyHttpApi::test_from_env();
        let gas_saved = simulate_before_after_access_list(
            &web3,
            &*tenderly_api,
            "1".to_string(),
            transaction_hash,
        )
        .await
        .unwrap();

        dbg!(gas_saved);
    }

    #[test]
    fn calldata_works() {
        let settlement = EncodedSettlement::default();
        let data = call_data(settlement);
        assert!(!data.is_empty());
    }
}

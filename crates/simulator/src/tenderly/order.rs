use std::sync::Arc;

use alloy_primitives::{Address, U256, address, map::AddressMap};
use alloy_rpc_types::{TransactionInput, TransactionRequest, state::{AccountOverride, StateOverride}};
use alloy_sol_types::SolCall;
use balance_overrides::{BalanceOverrideRequest, BalanceOverriding};
use contracts::alloy::{ICowWrapper::ICowWrapper, WETH9, support::AnyoneAuthenticator::AnyoneAuthenticator};
use model::{order::{BUY_ETH_ADDRESS, Order}, signature::{Signature, SigningScheme}};
use shared::{
    encoded_settlement::{
        EncodedSettlement, EncodedTrade, encode_trade, legacy_settlement_to_alloy,
    },
    interaction::{EncodedInteraction, Interaction},
};
use thiserror::Error;

use crate::infra::Ethereum;

type WrapperCalls = Vec<(Address, Vec<u8>)>;
const FAKE_SOLVER: Address = address!("0101010101010101010101010101010101010101");

pub struct SettlementSimulator {
    eth: Ethereum,
    native_token: Address,
    tenderly_simulator: ()
}

pub struct OrderSimulationTx {
    pub tx: TransactionRequest,
    pub state_override: StateOverride,
    pub uses_wrappers: bool
}

impl SettlementSimulator {

}

pub async fn order_transaction_request(order: &Order, eth: Ethereum) -> Result<OrderSimulationTx, Error> {
    let (settlement, wrappers) = build_settlement(order, todo!())?;
    let state_override = build_state_override(order, eth, eth.balance_overrider()).await?;
    let settle_calldata = legacy_settlement_to_alloy(settlement).abi_encode();

    let (call_target, final_calldata, uses_wrappers) = if !wrappers.is_empty() {
    let wrapper_data = encode_wrapper_data(&wrappers);
    let wrapped_calldata = ICowWrapper::wrappedSettleCall {
            settleData: settle_calldata.into(),
            wrapperData: wrapper_data.into()
        }.abi_encode();

        (wrappers[0].0, wrapped_calldata, true)
    } else {
        (order.metadata.settlement_contract, settle_calldata, false)
    };

    Ok(
        OrderSimulationTx {
        tx: TransactionRequest {
        from: Some(FAKE_SOLVER),
        to: Some(call_target.into()),
        input: TransactionInput::new(final_calldata.into()),
        gas: Some(10_000_000),
        ..Default::default()
    },
    state_override,
    uses_wrappers
    })
}

pub async fn encode_order_as_trade(order: &Order) -> EncodedTrade {
    let fake_signature = Signature::default_with(SigningScheme::Eip1271);
    encode_trade(&order.data, &fake_signature, order.metadata.owner, 0, 1, order.data.sell_amount)
}

pub async fn trade_simulation() -> () {

}

fn build_settlement(order: &Order, native_token: Address) -> Result<(EncodedSettlement, WrapperCalls), Error> {
    let tokens = vec![order.data.sell_token, order.data.buy_token];
    let clearing_prices = vec![order.data.buy_amount, order.data.sell_amount];

    let trade = encode_trade(&order.data, &order.signature, order.metadata.owner, 0, 1, U256::ZERO);
    let mut pre_hooks: Vec<EncodedInteraction> = order.interactions.pre.iter().map(Interaction::encode).collect();

    if order.data.buy_token == BUY_ETH_ADDRESS {
        // Because the `driver` manages `WETH` unwraps under the hood the `TradeFinder`
        // does not have to emit unwraps to pay out `ETH` in a trade.
        // However, for the simulation to be successful this has to happen so we do it
        // ourselves here.
        /*let buy_amount = match query.kind {
            OrderKind::Sell => *out_amount,
            OrderKind::Buy => query.in_amount.get(),
        };*/
        pre_hooks.push((
            native_token,
            U256::ZERO,
            WETH9::WETH9::withdrawCall { wad: order.data.buy_amount }
                .abi_encode()
                .into(),
        ));
        tracing::trace!("adding unwrap interaction for paying out ETH");
    }

    let post_hooks: Vec<EncodedInteraction> = order.interactions.post.iter().map(Interaction::encode).collect();
    let settlement = EncodedSettlement {
        tokens,
        clearing_prices,
        trades: vec![trade],
        interactions: [pre_hooks, vec![], post_hooks]
    };
    let wrappers = parse_wrappers(order)?;
    Ok((settlement, wrappers))
}

async fn build_state_override(order: &Order, eth: Ethereum, balance_overrides: Arc<dyn BalanceOverriding>) -> Result<StateOverride, Error> {
    let mut overrides: AddressMap<AccountOverride> = AddressMap::default();
    
    let authenticator = eth.contracts().settlement()
        .authenticator()
        .call()
        .await?;

    overrides.insert(
        authenticator,
        AccountOverride {
            code: Some(
                AnyoneAuthenticator::DEPLOYED_BYTECODE.clone()
            ),
            ..Default::default()
        }
    );

    // Give the fake solver some ETH balance
    overrides.insert(
        FAKE_SOLVER,
        AccountOverride {
            balance: Some(U256::from(1_000_000_000_000_000_000u128)),
            ..Default::default()
        },
    );

    // Fund the settlement contract with enough buy tokens to pay out
    // if let Some((token_address, balance_overrides)) = self.
    if let Some((token_addr, balance_override)) = 
        balance_overrides
        .state_override(BalanceOverrideRequest {
            token: order.data.buy_token,
            holder: *eth.contracts().settlement().address(),
            amount: order.data.buy_amount
        }).await {
            overrides.insert(token_addr, balance_override);
        }

    Ok(overrides)
}

/// Parse wrapper calls from the order's fullAppData.
fn parse_wrappers(order: &Order) -> Result<WrapperCalls, Error> {
    let Some(full_app_data) = &order.metadata.full_app_data else {
        return Ok(vec![]);
    };
    let root = serde_json::from_str::<app_data::Root>(full_app_data)?;
    let Some(metadata) = root.metadata() else { return Ok(vec![]) };

    Ok(metadata
        .wrappers
        .iter()
        .map(|w| (w.address, w.data.clone()))
        .collect())
}

fn encode_wrapper_data(wrappers: &WrapperCalls) -> Vec<u8> {
    let mut result = Vec::new();
    for (index, (address, data)) in wrappers.iter().enumerate() {
        if index != 0 {
            result.extend(address.as_slice());
        }
        result.extend((data.len() as u16).to_be_bytes());
        result.extend(data);
    }
    result
}

#[derive(Debug, Error)]
#[error("order simulation request error")]
pub enum Error {
    AppData(#[from] serde_json::Error),
    Alloy(#[from] alloy_contract::Error),
    Other(#[from] anyhow::Error),
}
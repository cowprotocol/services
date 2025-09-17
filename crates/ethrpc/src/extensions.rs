//! Module containing Ethereum RPC extension methods.

use {
    ethcontract::{
        BlockNumber,
        contract::{MethodBuilder, ViewMethodBuilder},
        errors::MethodError,
        tokens::Tokenize,
        transaction::TransactionBuilder,
    },
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    web3::{
        self,
        Transport,
        api::Namespace,
        ethabi::{Function, Token},
        helpers::{self, CallFuture},
        types::{BlockId, Bytes, CallRequest, H160, H256, U64, U256},
    },
};

/// Web3 convenience extension trait.
pub trait EthExt<T>
where
    T: Transport,
{
    fn call_with_state_overrides(
        &self,
        call: CallRequest,
        block: BlockId,
        overrides: HashMap<H160, StateOverride>,
    ) -> CallFuture<Bytes, T::Out>;
}

impl<T> EthExt<T> for web3::api::Eth<T>
where
    T: Transport,
{
    fn call_with_state_overrides(
        &self,
        call: CallRequest,
        block: BlockId,
        overrides: StateOverrides,
    ) -> CallFuture<Bytes, T::Out> {
        let call = helpers::serialize(&call);
        let block = helpers::serialize(&block);
        let overrides = helpers::serialize(&overrides);

        CallFuture::new(
            self.transport()
                .execute("eth_call", vec![call, block, overrides]),
        )
    }
}

pub trait CallBuilderExt<T, R>
where
    T: Transport,
    R: Tokenize,
{
    fn call_with_state_overrides(
        self,
        web3: &T,
        overrides: HashMap<H160, StateOverride>,
    ) -> impl Future<Output = Result<R, MethodError>>;
}

impl<T, R> CallBuilderExt<T, R> for MethodBuilder<T, R>
where
    T: Transport,
    R: Tokenize,
{
    async fn call_with_state_overrides(
        self,
        web3: &T,
        overrides: HashMap<H160, StateOverride>,
    ) -> Result<R, MethodError> {
        let function = self.function();
        let call = tx_builder_into_call_request(&self.tx);
        let block = BlockId::Number(BlockNumber::Latest);
        let future = web3::api::Eth::new(web3).call_with_state_overrides(call, block, overrides);
        convert_response::<_, R>(future, function).await
    }
}

impl<T, R> CallBuilderExt<T, R> for ViewMethodBuilder<T, R>
where
    T: Transport,
    R: Tokenize,
{
    async fn call_with_state_overrides(
        self,
        web3: &T,
        overrides: HashMap<H160, StateOverride>,
    ) -> Result<R, MethodError> {
        let function = self.function();
        let call = tx_builder_into_call_request(&self.m.tx);
        let block = self.block.unwrap_or(BlockId::Number(BlockNumber::Latest));
        let future = web3::api::Eth::new(web3).call_with_state_overrides(call, block, overrides);
        convert_response::<_, R>(future, function).await
    }
}

async fn convert_response<
    F: std::future::Future<Output = Result<Bytes, web3::Error>>,
    R: Tokenize,
>(
    future: F,
    function: &Function,
) -> Result<R, MethodError> {
    let bytes = future
        .await
        .map_err(|err| MethodError::new(function, err))?;
    let tokens = function
        .decode_output(&bytes.0)
        .map_err(|err| MethodError::new(function, err))?;
    let token = match tokens.len() {
        0 => Token::Tuple(Vec::new()),
        1 => tokens.into_iter().next().unwrap(),
        // Older versions of solc emit a list of tokens as the return type of functions returning
        // tuples instead of a single type that is a tuple. In order to be backwards compatible we
        // accept this too.
        _ => Token::Tuple(tokens),
    };
    let result = R::from_token(token).map_err(|err| MethodError::new(function, err))?;
    Ok(result)
}

pub fn tx_builder_into_call_request(tx: &TransactionBuilder<impl Transport>) -> CallRequest {
    let resolved_gas_price = tx
        .gas_price
        .map(|gas_price| gas_price.resolve_for_transaction())
        .unwrap_or_default();

    CallRequest {
        from: tx.from.as_ref().map(|account| account.address()),
        to: tx.to,
        gas: tx.gas,
        gas_price: resolved_gas_price.gas_price,
        value: tx.value,
        data: tx.data.clone(),
        transaction_type: resolved_gas_price.transaction_type,
        access_list: tx.access_list.clone(),
        max_fee_per_gas: resolved_gas_price.max_fee_per_gas,
        max_priority_fee_per_gas: resolved_gas_price.max_priority_fee_per_gas,
    }
}

/// State overrides.
pub type StateOverrides = HashMap<H160, StateOverride>;

/// State override object.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateOverride {
    /// Fake balance to set for the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<U256>,

    /// Fake nonce to set for the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<U64>,

    /// Fake EVM bytecode to inject into the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Bytes>,

    /// Fake key-value mapping to override **all** slots in the account storage
    /// before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<HashMap<H256, H256>>,

    /// Fake key-value mapping to override **individual** slots in the account
    /// storage before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_diff: Option<HashMap<H256, H256>>,
}

/// Debug namespace extension trait.
pub trait DebugNamespace<T>
where
    T: Transport,
{
    fn debug(&self) -> Debug<T>;
}

impl<T: Transport> DebugNamespace<T> for web3::Web3<T> {
    fn debug(&self) -> Debug<T> {
        self.api()
    }
}

/// `Debug` namespace
#[derive(Debug, Clone)]
pub struct Debug<T> {
    transport: T,
}

impl<T: Transport> Namespace<T> for Debug<T> {
    fn new(transport: T) -> Self
    where
        Self: Sized,
    {
        Debug { transport }
    }

    fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T: Transport> Debug<T> {
    /// Returns all debug traces for callTracer type of tracer.
    pub fn transaction(&self, hash: H256) -> CallFuture<CallFrame, T::Out> {
        let hash = helpers::serialize(&hash);
        let tracing_options = serde_json::json!({ "tracer": "callTracer" });
        CallFuture::new(
            self.transport()
                .execute("debug_traceTransaction", vec![hash, tracing_options]),
        )
    }
}

/// Taken from alloy::rpc::types::trace::geth::CallFrame
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct CallFrame {
    /// The address of that initiated the call.
    pub from: primitive_types::H160,
    /// The address of the contract that was called.
    #[serde(default)]
    pub to: Option<primitive_types::H160>,
    /// Calldata input.
    pub input: Bytes,
    /// Recorded child calls.
    #[serde(default)]
    pub calls: Vec<CallFrame>,
}

#[cfg(test)]
mod tests {
    use {super::*, crate::Web3, hex_literal::hex, maplit::hashmap, web3::types::BlockNumber};

    #[ignore]
    #[tokio::test]
    async fn can_call_with_state_override() {
        let web3 = Web3::new_from_env();

        let address = H160(hex!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"));
        let output = web3
            .eth()
            .call_with_state_overrides(
                CallRequest {
                    to: Some(address),
                    ..Default::default()
                },
                BlockNumber::Latest.into(),
                hashmap! {
                    address => StateOverride {
                        // EVM program to just return 32 bytes from 0 to 31
                        code: Some(hex!(
                            "7f 000102030405060708090a0b0c0d0e0f
                                101112131415161718191a1b1c1d1e1f
                             60 00
                             52
                             60 20
                             60 00
                             f3"
                        ).into()),
                        ..Default::default()
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(output.0, (0..32).collect::<Vec<_>>());
    }

    #[ignore]
    #[tokio::test]
    async fn method_builder_with_state_overrides() {
        let web3 = Web3::new_from_env();

        let weth = H160(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
        let weth = contracts::WETH9::at(&web3, weth);

        let trader = H160(hex!("1111111111111111111111111111111111111111"));
        let balance_storage_slot = H256(hex!(
            "fc40ea33816453f766ebc0872d4b5152b468882abe7b6b35528069db4d6e41c4"
        ));
        let faked_balance = U256::exp10(18);
        let faked_balance_as_fixed_bytes = {
            let mut buf = [0u8; 32];
            faked_balance.to_big_endian(&mut buf);
            H256(buf)
        };

        let balance = weth
            .balance_of(trader)
            .call_with_state_overrides(
                web3.transport(),
                hashmap! {
                    weth.address() => StateOverride {
                        state: Some(hashmap! {
                            balance_storage_slot => faked_balance_as_fixed_bytes,
                        }),
                        ..Default::default()
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(balance, faked_balance);
    }
}

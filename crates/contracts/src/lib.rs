#![allow(clippy::let_unit_value)]
#[cfg(feature = "bin")]
pub mod paths;
pub mod vault;

macro_rules! include_contracts {
    ($($name:ident;)*) => {$(
        include!(concat!(env!("OUT_DIR"), "/", stringify!($name), ".rs"));
    )*};
}

include_contracts! {
    BalancerV2Authorizer;
    BalancerV2BasePool;
    BalancerV2BasePoolFactory;
    BalancerV2LiquidityBootstrappingPool;
    BalancerV2LiquidityBootstrappingPoolFactory;
    BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory;
    BalancerV2StablePool;
    BalancerV2StablePoolFactory;
    BalancerV2StablePoolFactoryV2;
    BalancerV2Vault;
    BalancerV2WeightedPool2TokensFactory;
    BalancerV2WeightedPool;
    BalancerV2WeightedPoolFactory;
    BaoswapFactory;
    BaoswapRouter;
    CowProtocolToken;
    CowProtocolVirtualToken;
    CoWSwapOnchainOrders;
    ERC1271SignatureValidator;
    ERC20;
    ERC20Mintable;
    GPv2AllowListAuthentication;
    GPv2Settlement;
    GnosisSafe;
    GnosisSafeCompatibilityFallbackHandler;
    GnosisSafeProxy;
    HoneyswapFactory;
    HoneyswapRouter;
    ISwaprPair;
    IUniswapLikePair;
    IUniswapLikeRouter;
    IUniswapV3Factory;
    IZeroEx;
    SushiSwapFactory;
    SushiSwapRouter;
    SwaprFactory;
    SwaprRouter;
    UniswapV2Factory;
    UniswapV2Router02;
    UniswapV3SwapRouter;
    WETH9;
}

pub mod support {
    include_contracts! {
        AnyoneAuthenticator;
        PhonyERC20;
        Trader;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::{
        common::DeploymentInformation,
        futures::future::{self, FutureExt as _, Ready},
        json::json,
        jsonrpc::{Call, Id, MethodCall, Params, Value},
        web3::{error::Result as Web3Result, BatchTransport, RequestId, Transport, Web3},
    };

    #[derive(Debug, Clone)]
    struct ChainIdTransport(u64);

    impl Transport for ChainIdTransport {
        type Out = Ready<Web3Result<Value>>;

        fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
            assert_eq!(method, "net_version");
            assert_eq!(params.len(), 0);
            (
                0,
                MethodCall {
                    jsonrpc: None,
                    method: method.to_string(),
                    params: Params::Array(params),
                    id: Id::Num(0),
                }
                .into(),
            )
        }

        fn send(&self, _id: RequestId, _request: Call) -> Self::Out {
            future::ready(Ok(json!(format!("{}", self.0))))
        }
    }

    impl BatchTransport for ChainIdTransport {
        type Batch = Ready<Web3Result<Vec<Web3Result<Value>>>>;

        fn send_batch<T>(&self, requests: T) -> Self::Batch
        where
            T: IntoIterator<Item = (RequestId, Call)>,
        {
            future::ready(Ok(requests
                .into_iter()
                .map(|_| Ok(json!(format!("{}", self.0))))
                .collect()))
        }
    }

    #[test]
    fn deployment_addresses() {
        macro_rules! assert_has_deployment_address {
            ($contract:ident for $network:expr) => {{
                let web3 = Web3::new(ChainIdTransport($network));
                let deployed = $contract::deployed(&web3).now_or_never().unwrap();
                assert!(deployed.is_ok());
            }};
        }

        for network in &[1, 4, 100] {
            assert_has_deployment_address!(GPv2Settlement for *network);
            assert_has_deployment_address!(SushiSwapFactory for *network);
            assert_has_deployment_address!(SushiSwapRouter for *network);
            assert_has_deployment_address!(WETH9 for *network);
        }
        for network in &[1, 4] {
            assert_has_deployment_address!(BalancerV2Vault for *network);
            assert_has_deployment_address!(BalancerV2LiquidityBootstrappingPoolFactory for *network);
            assert_has_deployment_address!(BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory for *network);
            assert_has_deployment_address!(BalancerV2WeightedPoolFactory for *network);
            assert_has_deployment_address!(BalancerV2WeightedPool2TokensFactory for *network);
            assert_has_deployment_address!(BalancerV2StablePoolFactory for *network);
            assert_has_deployment_address!(UniswapV2Factory for *network);
            assert_has_deployment_address!(UniswapV2Router02 for *network);
        }
        #[allow(clippy::single_element_loop)]
        for network in &[100] {
            assert_has_deployment_address!(HoneyswapFactory for *network);
            assert_has_deployment_address!(HoneyswapRouter for *network);
        }
        assert_has_deployment_address!(BalancerV2StablePoolFactoryV2 for 1);
        assert_has_deployment_address!(UniswapV3SwapRouter for 1);
    }

    #[test]
    fn deployment_information() {
        macro_rules! assert_has_deployment_information {
            ($contract:ident for $network:expr) => {{
                let web3 = Web3::new(ChainIdTransport($network));
                let instance = $contract::deployed(&web3).now_or_never().unwrap().unwrap();
                assert!(matches!(
                    instance.deployment_information(),
                    Some(DeploymentInformation::BlockNumber(_)),
                ));
            }};
        }

        for network in &[1, 4, 100] {
            assert_has_deployment_information!(GPv2Settlement for *network);
        }
        for network in &[1, 4] {
            assert_has_deployment_information!(BalancerV2Vault for *network);
            assert_has_deployment_information!(BalancerV2WeightedPoolFactory for *network);
            assert_has_deployment_information!(BalancerV2WeightedPool2TokensFactory for *network);
            assert_has_deployment_information!(BalancerV2StablePoolFactory for *network);
        }
        assert_has_deployment_information!(BalancerV2StablePoolFactoryV2 for 1);
    }

    #[test]
    fn bytecode() {
        macro_rules! assert_has_bytecode {
            ($contract:ty) => {{
                let contract = <$contract>::raw_contract();
                assert!(!contract.bytecode.is_empty());
            }};
        }

        assert_has_bytecode!(support::AnyoneAuthenticator);
        assert_has_bytecode!(support::PhonyERC20);
        assert_has_bytecode!(support::Trader);
    }
}

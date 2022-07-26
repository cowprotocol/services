#![allow(clippy::let_unit_value)]
#[cfg(feature = "bin")]
pub mod paths;
pub mod vault;

include!(concat!(env!("OUT_DIR"), "/BalancerV2Authorizer.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2BasePool.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2BasePoolFactory.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2LiquidityBootstrappingPool.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2LiquidityBootstrappingPoolFactory.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory.rs"
));
include!(concat!(env!("OUT_DIR"), "/BalancerV2StablePool.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2StablePoolFactory.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2StablePoolFactoryV2.rs"
));
include!(concat!(env!("OUT_DIR"), "/BalancerV2Vault.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2WeightedPool.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2WeightedPool2TokensFactory.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2WeightedPoolFactory.rs"
));
include!(concat!(env!("OUT_DIR"), "/BaoswapFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/BaoswapRouter.rs"));
include!(concat!(env!("OUT_DIR"), "/ERC20.rs"));
include!(concat!(env!("OUT_DIR"), "/ERC20Mintable.rs"));
include!(concat!(env!("OUT_DIR"), "/GPv2AllowListAuthentication.rs"));
include!(concat!(env!("OUT_DIR"), "/GPv2Settlement.rs"));
include!(concat!(env!("OUT_DIR"), "/GnosisSafe.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/GnosisSafeCompatibilityFallbackHandler.rs"
));
include!(concat!(env!("OUT_DIR"), "/GnosisSafeProxy.rs"));
include!(concat!(env!("OUT_DIR"), "/HoneyswapFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/HoneyswapRouter.rs"));
include!(concat!(env!("OUT_DIR"), "/IUniswapLikePair.rs"));
include!(concat!(env!("OUT_DIR"), "/IUniswapLikeRouter.rs"));
include!(concat!(env!("OUT_DIR"), "/ERC1271SignatureValidator.rs"));
include!(concat!(env!("OUT_DIR"), "/SushiSwapFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/SushiSwapRouter.rs"));
include!(concat!(env!("OUT_DIR"), "/SwaprFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/SwaprRouter.rs"));
include!(concat!(env!("OUT_DIR"), "/ISwaprPair.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Factory.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Router02.rs"));
include!(concat!(env!("OUT_DIR"), "/WETH9.rs"));
include!(concat!(env!("OUT_DIR"), "/IUniswapV3Factory.rs"));
include!(concat!(env!("OUT_DIR"), "/IZeroEx.rs"));
include!(concat!(env!("OUT_DIR"), "/CowProtocolToken.rs"));
include!(concat!(env!("OUT_DIR"), "/CowProtocolVirtualToken.rs"));

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
}

#[cfg(feature = "bin")]
pub mod paths;
pub mod vault;

include!(concat!(env!("OUT_DIR"), "/BalancerV2Authorizer.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2Vault.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2WeightedPool.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2WeightedPoolFactory.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2WeightedPool2TokensFactory.rs"
));
include!(concat!(env!("OUT_DIR"), "/ERC20.rs"));
include!(concat!(env!("OUT_DIR"), "/ERC20Mintable.rs"));
include!(concat!(env!("OUT_DIR"), "/GPv2AllowListAuthentication.rs"));
include!(concat!(env!("OUT_DIR"), "/GPv2Settlement.rs"));
include!(concat!(env!("OUT_DIR"), "/IUniswapLikePair.rs"));
include!(concat!(env!("OUT_DIR"), "/IUniswapLikeRouter.rs"));
include!(concat!(env!("OUT_DIR"), "/SushiswapV2Factory.rs"));
include!(concat!(env!("OUT_DIR"), "/SushiswapV2Router02.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Factory.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Router02.rs"));
include!(concat!(env!("OUT_DIR"), "/WETH9.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::{
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
            assert_has_deployment_address!(SushiswapV2Factory for *network);
            assert_has_deployment_address!(SushiswapV2Router02 for *network);
            assert_has_deployment_address!(UniswapV2Factory for *network);
            assert_has_deployment_address!(UniswapV2Router02 for *network);
            assert_has_deployment_address!(WETH9 for *network);
        }
        for network in &[1, 4] {
            assert_has_deployment_address!(BalancerV2Vault for *network);
            assert_has_deployment_address!(BalancerV2WeightedPoolFactory for *network);
            assert_has_deployment_address!(BalancerV2WeightedPool2TokensFactory for *network);
        }
    }

    #[test]
    fn deployment_information() {
        macro_rules! assert_has_deployment_information {
            ($contract:ident for $network:expr) => {{
                let web3 = Web3::new(ChainIdTransport($network));
                let instance = $contract::deployed(&web3).now_or_never().unwrap().unwrap();
                assert!(instance.deployment_information().is_some());
            }};
        }

        for network in &[1, 4, 100] {
            assert_has_deployment_information!(GPv2Settlement for *network);
        }
        for network in &[1, 4] {
            assert_has_deployment_information!(BalancerV2Vault for *network);
            assert_has_deployment_information!(BalancerV2WeightedPoolFactory for *network);
            assert_has_deployment_information!(BalancerV2WeightedPool2TokensFactory for *network);
        }
    }
}

#![allow(clippy::let_unit_value)]

pub use ethcontract;

#[macro_use]
mod macros;

#[cfg(feature = "bin")]
pub mod paths;
pub mod storage_accessible;
pub mod vault;
pub mod web3;

macro_rules! include_contracts {
    ($($name:ident;)*) => {$(
        include!(concat!(env!("OUT_DIR"), "/", stringify!($name), ".rs"));
    )*};
}

include_contracts! {
    BalancerV2Authorizer;
    BalancerV2BasePool;
    BalancerV2BasePoolFactory;
    BalancerV2ComposableStablePool;
    BalancerV2ComposableStablePoolFactory;
    BalancerV2ComposableStablePoolFactoryV3;
    BalancerV2ComposableStablePoolFactoryV4;
    BalancerV2ComposableStablePoolFactoryV5;
    BalancerV2LiquidityBootstrappingPool;
    BalancerV2LiquidityBootstrappingPoolFactory;
    BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory;
    BalancerV2StablePool;
    BalancerV2StablePoolFactoryV2;
    BalancerV2Vault;
    BalancerV2WeightedPool2TokensFactory;
    BalancerV2WeightedPool;
    BalancerV2WeightedPoolFactory;
    BalancerV2WeightedPoolFactoryV3;
    BalancerV2WeightedPoolFactoryV4;
    BaoswapRouter;
    CoWSwapEthFlow;
    CoWSwapOnchainOrders;
    CowProtocolToken;
    ERC1271SignatureValidator;
    ERC20;
    ERC20Mintable;
    GPv2AllowListAuthentication;
    GPv2Settlement;
    GnosisSafe;
    GnosisSafeCompatibilityFallbackHandler;
    GnosisSafeProxy;
    GnosisSafeProxyFactory;
    HoneyswapRouter;
    HooksTrampoline;
    ISwaprPair;
    IUniswapLikePair;
    IUniswapLikeRouter;
    IUniswapV3Factory;
    IZeroEx;
    PancakeRouter;
    ChainalysisOracle;
    SushiSwapRouter;
    SwaprRouter;
    TestnetUniswapV2Router02;
    UniswapV2Factory;
    UniswapV2Router02;
    UniswapV3Pool;
    UniswapV3SwapRouter;
    WETH9;
}

pub mod support {
    include_contracts! {
        AnyoneAuthenticator;
        Balances;
        FetchBlock;
        Multicall;
        Signatures;
        SimulateCode;
        Solver;
        Swapper;
        Trader;
    }
}

pub mod test {
    include_contracts! {
        Counter;
        GasHog;
    }
}

#[cfg(test)]
mod tests {
    const MAINNET: u64 = 1;
    const GOERLI: u64 = 5;
    const GNOSIS: u64 = 100;
    const SEPOLIA: u64 = 11155111;
    const ARBITRUM: u64 = 42161;

    use {
        super::*,
        ethcontract::{
            common::DeploymentInformation,
            futures::future::{self, FutureExt as _, Ready},
            json::json,
            jsonrpc::{Call, Id, MethodCall, Params, Value},
            web3::{error::Result as Web3Result, BatchTransport, RequestId, Transport, Web3},
        },
    };

    #[derive(Debug, Clone)]
    struct ChainIdTransport(u64);

    impl Transport for ChainIdTransport {
        type Out = Ready<Web3Result<Value>>;

        fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
            assert_eq!(method, "eth_chainId");
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
            future::ready(Ok(json!(format!("{:x}", self.0))))
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
                .map(|_| Ok(json!(format!("{:x}", self.0))))
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

        for network in &[MAINNET, GOERLI, GNOSIS, SEPOLIA, ARBITRUM] {
            assert_has_deployment_address!(GPv2Settlement for *network);
            assert_has_deployment_address!(WETH9 for *network);
            assert_has_deployment_address!(HooksTrampoline for *network);
            assert_has_deployment_address!(BalancerV2Vault for *network);
            assert_has_deployment_address!(BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory for *network);
        }
        for network in &[MAINNET, GOERLI, GNOSIS, SEPOLIA] {
            assert_has_deployment_address!(CowProtocolToken for *network);
        }
        for network in &[MAINNET, GOERLI, GNOSIS, ARBITRUM] {
            assert_has_deployment_address!(SushiSwapRouter for *network);
            assert_has_deployment_address!(UniswapV2Factory for *network);
            assert_has_deployment_address!(UniswapV2Router02 for *network);
        }
        for network in &[MAINNET, GOERLI, SEPOLIA, ARBITRUM] {
            assert_has_deployment_address!(UniswapV3SwapRouter for *network);
            assert_has_deployment_address!(IUniswapV3Factory for *network);
        }
        for network in &[MAINNET, GOERLI, ARBITRUM] {
            assert_has_deployment_address!(BalancerV2WeightedPool2TokensFactory for *network);
            assert_has_deployment_address!(BalancerV2LiquidityBootstrappingPoolFactory for *network);
        }

        for network in &[MAINNET, ARBITRUM] {
            assert_has_deployment_address!(PancakeRouter for *network);
        }

        for network in &[MAINNET, GOERLI] {
            assert_has_deployment_address!(BalancerV2WeightedPoolFactory for *network);
        }

        for network in &[MAINNET, SEPOLIA, ARBITRUM] {
            assert_has_deployment_address!(IZeroEx for *network);
        }

        for network in &[MAINNET, GOERLI, GNOSIS, ARBITRUM] {
            assert_has_deployment_address!(BalancerV2StablePoolFactoryV2 for *network);
        }

        for network in &[MAINNET, GNOSIS, ARBITRUM] {
            assert_has_deployment_address!(SwaprRouter for *network);
        }

        // only gnosis
        assert_has_deployment_address!(BaoswapRouter for GNOSIS);
        assert_has_deployment_address!(HoneyswapRouter for GNOSIS);

        // only sepolia
        assert_has_deployment_address!(TestnetUniswapV2Router02 for SEPOLIA);
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

        for network in &[MAINNET, GOERLI, GNOSIS, SEPOLIA, ARBITRUM] {
            assert_has_deployment_information!(GPv2Settlement for *network);
            assert_has_deployment_information!(BalancerV2Vault for *network);
        }
        for network in &[MAINNET, GOERLI] {
            assert_has_deployment_information!(BalancerV2WeightedPoolFactory for *network);
        }
        for network in &[MAINNET, GOERLI, ARBITRUM] {
            assert_has_deployment_information!(BalancerV2WeightedPool2TokensFactory for *network);
        }
        for network in &[MAINNET, GOERLI, GNOSIS, ARBITRUM] {
            assert_has_deployment_information!(BalancerV2StablePoolFactoryV2 for *network);
        }
    }

    #[test]
    fn bytecode() {
        macro_rules! assert_has_bytecode {
            ($contract:ty) => {{
                let contract = <$contract>::raw_contract();
                assert!(!contract.bytecode.is_empty());
            }};
        }

        assert_has_bytecode!(support::Trader);
        assert_has_bytecode!(support::Solver);
    }
}

//! End-to-end tests for `pool-indexer`: on-chain cold-seed and catch-up, the
//! HTTP API, and the driver consuming pool data from it.

use {
    alloy::{
        primitives::{
            Address,
            U256,
            aliases::{I24, U24, U160},
        },
        providers::Provider,
        sol,
        sol_types::SolEvent,
    },
    e2e::setup::{OnchainComponents, TIMEOUT, colocation, run_test, wait_for_condition},
    ethrpc::Web3,
    number::units::EthUnit,
    pool_indexer::config::{
        ApiConfig,
        BalancerV2Config,
        Configuration,
        DatabaseConfig,
        FactoryConfig,
        MetricsConfig,
        NetworkConfig,
        NetworkName,
        UniswapV3Config,
    },
    serde::Deserialize,
    sqlx::PgPool,
    std::{
        future::Future,
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        num::NonZeroU32,
    },
};

// Mock V3 factory. Bytecode compiled from the .sol source below with solc
// 0.8.30 --optimize --optimize-runs 1000000, evm-version shanghai.
//
// // SPDX-License-Identifier: GPL-3.0-or-later
// pragma solidity ^0.8.17;
// import "./MockUniswapV3Pool.sol";
// contract MockUniswapV3Factory {
//     event PoolCreated(
//         address indexed token0, address indexed token1, uint24 indexed fee,
//         int24 tickSpacing, address pool
//     );
//     function createPool(address tokenA, address tokenB, uint24 _fee)
//         external returns (address pool)
//     {
//         (address t0, address t1) =
//             tokenA < tokenB ? (tokenA, tokenB) : (tokenB, tokenA);
//         MockUniswapV3Pool p = new MockUniswapV3Pool(t0, t1, _fee);
//         pool = address(p);
//         emit PoolCreated(t0, t1, _fee, int24(10), pool);
//     }
// }
sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode = "0x6080604052348015600e575f5ffd5b506106dd8061001c5f395ff3fe608060405234801561000f575f5ffd5b5060043610610029575f3560e01c8063a16712951461002d575b5f5ffd5b61004061003b3660046101ab565b610069565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200160405180910390f35b5f5f5f8473ffffffffffffffffffffffffffffffffffffffff168673ffffffffffffffffffffffffffffffffffffffff16106100a65784866100a9565b85855b915091505f8282866040516100bd90610176565b73ffffffffffffffffffffffffffffffffffffffff938416815292909116602083015262ffffff166040820152606001604051809103905ff080158015610106573d5f5f3e3d5ffd5b5060408051600a815273ffffffffffffffffffffffffffffffffffffffff808416602083015292965086935062ffffff88169280861692908716917f783cca1c0412dd0d695e784568c96da2e9c22ff989357a2e8b1d9b2b4e6b7118910160405180910390a45050509392505050565b6104da806101f783390190565b803573ffffffffffffffffffffffffffffffffffffffff811681146101a6575f5ffd5b919050565b5f5f5f606084860312156101bd575f5ffd5b6101c684610183565b92506101d460208501610183565b9150604084013562ffffff811681146101eb575f5ffd5b80915050925092509256fe60e060405234801561000f575f5ffd5b506040516104da3803806104da83398101604081905261002e91610069565b6001600160a01b03928316608052911660a05262ffffff1660c0526100b4565b80516001600160a01b0381168114610064575f5ffd5b919050565b5f5f5f6060848603121561007b575f5ffd5b6100848461004e565b92506100926020850161004e565b9150604084015162ffffff811681146100a9575f5ffd5b809150509250925092565b60805160a05160c0516103fd6100dd5f395f61012c01525f61010501525f607801526103fd5ff3fe608060405234801561000f575f5ffd5b506004361061006f575f3560e01c8063ddca3f431161004d578063ddca3f4314610127578063efe27fa314610162578063f637731d14610177575f5ffd5b80630dfe1681146100735780631a686502146100c4578063d21220a714610100575b5f5ffd5b61009a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b5f546100df906fffffffffffffffffffffffffffffffff1681565b6040516fffffffffffffffffffffffffffffffff90911681526020016100bb565b61009a7f000000000000000000000000000000000000000000000000000000000000000081565b61014e7f000000000000000000000000000000000000000000000000000000000000000081565b60405162ffffff90911681526020016100bb565b610175610170366004610312565b61018a565b005b61017561018536600461037b565b610287565b5f805482919081906101af9084906fffffffffffffffffffffffffffffffff1661039d565b92506101000a8154816fffffffffffffffffffffffffffffffff02191690836fffffffffffffffffffffffffffffffff1602179055508160020b8360020b8573ffffffffffffffffffffffffffffffffffffffff167f7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde33855f5f604051610279949392919073ffffffffffffffffffffffffffffffffffffffff9490941684526fffffffffffffffffffffffffffffffff9290921660208401526040830152606082015260800190565b60405180910390a450505050565b6040805173ffffffffffffffffffffffffffffffffffffffff831681525f60208201527f98636036cb66a9c19a37435efc1e90142190214e8abeb821bdba3f2990dd4c95910160405180910390a150565b73ffffffffffffffffffffffffffffffffffffffff811681146102f9575f5ffd5b50565b8035600281900b811461030d575f5ffd5b919050565b5f5f5f5f60808587031215610325575f5ffd5b8435610330816102d8565b935061033e602086016102fc565b925061034c604086016102fc565b915060608501356fffffffffffffffffffffffffffffffff81168114610370575f5ffd5b939692955090935050565b5f6020828403121561038b575f5ffd5b8135610396816102d8565b9392505050565b6fffffffffffffffffffffffffffffffff81811683821601908111156103ea577f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b9291505056fea164736f6c634300081e000aa164736f6c634300081e000a")]
    contract MockUniswapV3Factory {
        event PoolCreated(
            address indexed token0,
            address indexed token1,
            uint24  indexed fee,
            int24           tickSpacing,
            address         pool
        );

        function createPool(
            address tokenA,
            address tokenB,
            uint24  _fee
        ) external returns (address pool);
    }
}

// Mock V3 pool. Compiled identically to the factory above.
//
// // SPDX-License-Identifier: GPL-3.0-or-later
// pragma solidity ^0.8.17;
// contract MockUniswapV3Pool {
//     address public immutable token0;
//     address public immutable token1;
//     uint24  public immutable fee;
//     uint128 public liquidity;
//     event Initialize(uint160 sqrtPriceX96, int24 tick);
//     event Mint(
//         address sender, address indexed owner,
//         int24 indexed tickLower, int24 indexed tickUpper,
//         uint128 amount, uint256 amount0, uint256 amount1
//     );
//     constructor(address _token0, address _token1, uint24 _fee) {
//         token0 = _token0; token1 = _token1; fee = _fee;
//     }
//     function initialize(uint160 sqrtPriceX96) external {
//         emit Initialize(sqrtPriceX96, int24(0));
//     }
//     function mockMint(
//         address owner, int24 tickLower, int24 tickUpper, uint128 amount
//     ) external {
//         liquidity += amount;
//         emit Mint(msg.sender, owner, tickLower, tickUpper, amount, 0, 0);
//     }
// }
sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode = "0x60e060405234801561000f575f5ffd5b506040516104da3803806104da83398101604081905261002e91610069565b6001600160a01b03928316608052911660a05262ffffff1660c0526100b4565b80516001600160a01b0381168114610064575f5ffd5b919050565b5f5f5f6060848603121561007b575f5ffd5b6100848461004e565b92506100926020850161004e565b9150604084015162ffffff811681146100a9575f5ffd5b809150509250925092565b60805160a05160c0516103fd6100dd5f395f61012c01525f61010501525f607801526103fd5ff3fe608060405234801561000f575f5ffd5b506004361061006f575f3560e01c8063ddca3f431161004d578063ddca3f4314610127578063efe27fa314610162578063f637731d14610177575f5ffd5b80630dfe1681146100735780631a686502146100c4578063d21220a714610100575b5f5ffd5b61009a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b5f546100df906fffffffffffffffffffffffffffffffff1681565b6040516fffffffffffffffffffffffffffffffff90911681526020016100bb565b61009a7f000000000000000000000000000000000000000000000000000000000000000081565b61014e7f000000000000000000000000000000000000000000000000000000000000000081565b60405162ffffff90911681526020016100bb565b610175610170366004610312565b61018a565b005b61017561018536600461037b565b610287565b5f805482919081906101af9084906fffffffffffffffffffffffffffffffff1661039d565b92506101000a8154816fffffffffffffffffffffffffffffffff02191690836fffffffffffffffffffffffffffffffff1602179055508160020b8360020b8573ffffffffffffffffffffffffffffffffffffffff167f7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde33855f5f604051610279949392919073ffffffffffffffffffffffffffffffffffffffff9490941684526fffffffffffffffffffffffffffffffff9290921660208401526040830152606082015260800190565b60405180910390a450505050565b6040805173ffffffffffffffffffffffffffffffffffffffff831681525f60208201527f98636036cb66a9c19a37435efc1e90142190214e8abeb821bdba3f2990dd4c95910160405180910390a150565b73ffffffffffffffffffffffffffffffffffffffff811681146102f9575f5ffd5b50565b8035600281900b811461030d575f5ffd5b919050565b5f5f5f5f60808587031215610325575f5ffd5b8435610330816102d8565b935061033e602086016102fc565b925061034c604086016102fc565b915060608501356fffffffffffffffffffffffffffffffff81168114610370575f5ffd5b939692955090935050565b5f6020828403121561038b575f5ffd5b8135610396816102d8565b9392505050565b6fffffffffffffffffffffffffffffffff81811683821601908111156103ea577f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b9291505056fea164736f6c634300081e000a")]
    contract MockUniswapV3Pool {
        event Initialize(uint160 sqrtPriceX96, int24 tick);
        event Mint(
            address          sender,
            address indexed  owner,
            int24   indexed  tickLower,
            int24   indexed  tickUpper,
            uint128          amount,
            uint256          amount0,
            uint256          amount1
        );

        function initialize(uint160 sqrtPriceX96) external;
        function mockMint(
            address owner,
            int24   tickLower,
            int24   tickUpper,
            uint128 amount
        ) external;
    }
}

// Mock Balancer token. Bytecode compiled from the .sol below with solc 0.8.30,
// evm-version paris (no PUSH0, so it deploys at the node's pre-Shanghai
// genesis).
//
// contract MockToken {
//     uint8 public decimals;
//     constructor(uint8 d) { decimals = d; }
// }
sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode = "0x6080604052348015600f57600080fd5b506040516100ff3803806100ff833981016040819052602c916044565b6000805460ff191660ff92909216919091179055606c565b600060208284031215605557600080fd5b815160ff81168114606557600080fd5b9392505050565b60858061007a6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063313ce56714602d575b600080fd5b60005460399060ff1681565b60405160ff909116815260200160405180910390f3fea26469706673582212203e94d89ff9b8c4622833664d804ac0db01e8d6d8e741c93b5071047b13962bc364736f6c634300081e0033")]
    contract MockToken {
        constructor(uint8 d);
        function decimals() external view returns (uint8);
    }
}

// Mock Balancer Vault. Compiled identically.
//
// contract MockBalancerVault {
//     uint256 public nonce;
//     mapping(bytes32 => address[]) tokens;
//     mapping(bytes32 => uint256[]) balances;
//     function registerPool() external returns (bytes32 id) {
//         id = bytes32((uint256(uint160(msg.sender)) << 96) | nonce++);
//     }
//     function registerTokens(
//         bytes32 id, address[] memory t, uint256[] memory b
//     ) external { tokens[id] = t; balances[id] = b; }
//     function getPoolTokens(bytes32 id) external view
//         returns (address[] memory, uint256[] memory, uint256)
//     { return (tokens[id], balances[id], 0); }
// }
sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode = "0x6080604052348015600f57600080fd5b506106158061001f6000396000f3fe608060405234801561001057600080fd5b506004361061004c5760003560e01c80637b09f30314610051578063affed0e014610066578063d7740ee114610082578063f94d46681461008a575b600080fd5b61006461005f3660046103da565b6100ac565b005b61006f60005481565b6040519081526020015b60405180910390f35b61006f6100f1565b61009d6100983660046104c7565b61010e565b604051610079939291906104e0565b600083815260016020908152604090912083516100cb928501906101f1565b50600083815260026020908152604090912082516100eb9284019061027b565b50505050565b60008054818061010083610580565b909155503360601b17919050565b6000818152600160209081526040808320600283528184208154835181860281018601909452808452606095869590948592909185919083018282801561018b57602002820191906000526020600020905b815473ffffffffffffffffffffffffffffffffffffffff168152600190910190602001808311610160575b50505050509250818054806020026020016040519081016040528092919081815260200182805480156101dd57602002820191906000526020600020905b8154815260200190600101908083116101c9575b505050505091509250925092509193909250565b82805482825590600052602060002090810192821561026b579160200282015b8281111561026b57825182547fffffffffffffffffffffffff00000000000000000000000000000000000000001673ffffffffffffffffffffffffffffffffffffffff909116178255602090920191600190910190610211565b506102779291506102b6565b5090565b82805482825590600052602060002090810192821561026b579160200282015b8281111561026b57825182559160200191906001019061029b565b5b8082111561027757600081556001016102b7565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052604160045260246000fd5b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff81118282101715610341576103416102cb565b604052919050565b600067ffffffffffffffff821115610363576103636102cb565b5060051b60200190565b600082601f83011261037e57600080fd5b813561039161038c82610349565b6102fa565b8082825260208201915060208360051b8601019250858311156103b357600080fd5b602085015b838110156103d05780358352602092830192016103b8565b5095945050505050565b6000806000606084860312156103ef57600080fd5b83359250602084013567ffffffffffffffff81111561040d57600080fd5b8401601f8101861361041e57600080fd5b803561042c61038c82610349565b8082825260208201915060208360051b85010192508883111561044e57600080fd5b6020840193505b8284101561049257833573ffffffffffffffffffffffffffffffffffffffff8116811461048157600080fd5b825260209384019390910190610455565b9450505050604084013567ffffffffffffffff8111156104b157600080fd5b6104bd8682870161036d565b9150509250925092565b6000602082840312156104d957600080fd5b5035919050565b6060808252845190820181905260009060208601906080840190835b8181101561053057835173ffffffffffffffffffffffffffffffffffffffff168352602093840193909201916001016104fc565b50508381036020808601919091528651808352918101925086019060005b8181101561056c57825184526020938401939092019160010161054e565b505050604092909201929092529392505050565b60007fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82036105d8577f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b506001019056fea26469706673582212203197a76519149c61c64bfd3037184cfc3e2c3d6205ecff64443e2b0a2dec88de64736f6c634300081e0033")]
    contract MockBalancerVault {
        function getPoolTokens(bytes32 poolId)
            external view returns (address[] memory, uint256[] memory, uint256);
    }
}

// Mock Balancer weighted-pool factory + the pool it deploys. Compiled
// identically.
//
// contract MockBalancerPool {
//     bytes32 public poolId;
//     uint256[] weights;
//     constructor(
//         MockBalancerVault v, address[] memory t,
//         uint256[] memory w, uint256[] memory b
//     ) {
//         poolId = v.registerPool();
//         v.registerTokens(poolId, t, b);
//         weights = w;
//     }
//     function getPoolId() external view returns (bytes32) { return poolId; }
//     function getNormalizedWeights()
//         external view returns (uint256[] memory) { return weights; }
//     function getSwapFeePercentage()
//         external pure returns (uint256) { return 1e15; } // 0.1%
//     function getPausedState()
//         external pure returns (bool, uint256, uint256)
//     { return (false, 0, 0); }
// }
//
// contract MockBalancerPoolFactory {
//     MockBalancerVault vault;
//     event PoolCreated(address indexed pool);
//     constructor(MockBalancerVault v) { vault = v; }
//     function createPool(
//         address[] memory t, uint256[] memory w, uint256[] memory b
//     ) external returns (address pool) {
//         pool = address(new MockBalancerPool(vault, t, w, b));
//         emit PoolCreated(pool);
//     }
// }
sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode = "0x6080604052348015600f57600080fd5b50604051610ad1380380610ad1833981016040819052602c916050565b600080546001600160a01b0319166001600160a01b0392909216919091179055607e565b600060208284031215606157600080fd5b81516001600160a01b0381168114607757600080fd5b9392505050565b610a448061008d6000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80637ac1eb8e1461003b578063fbfa77cf14610077575b600080fd5b61004e610049366004610257565b610097565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200160405180910390f35b60005461004e9073ffffffffffffffffffffffffffffffffffffffff1681565b6000805460405173ffffffffffffffffffffffffffffffffffffffff909116908590859085906100c69061013b565b6100d394939291906103a2565b604051809103906000f0801580156100ef573d6000803e3d6000fd5b5060405190915073ffffffffffffffffffffffffffffffffffffffff8216907f83a48fbcfc991335314e74d0496aab6a1987e992ddc85dddbcc4d6dd6ef2e9fc90600090a29392505050565b6105c98061044683390190565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052604160045260246000fd5b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff811182821017156101be576101be610148565b604052919050565b600067ffffffffffffffff8211156101e0576101e0610148565b5060051b60200190565b600082601f8301126101fb57600080fd5b813561020e610209826101c6565b610177565b8082825260208201915060208360051b86010192508583111561023057600080fd5b602085015b8381101561024d578035835260209283019201610235565b5095945050505050565b60008060006060848603121561026c57600080fd5b833567ffffffffffffffff81111561028357600080fd5b8401601f8101861361029457600080fd5b80356102a2610209826101c6565b8082825260208201915060208360051b8501019250888311156102c457600080fd5b6020840193505b8284101561030857833573ffffffffffffffffffffffffffffffffffffffff811681146102f757600080fd5b8252602093840193909101906102cb565b9550505050602084013567ffffffffffffffff81111561032757600080fd5b610333868287016101ea565b925050604084013567ffffffffffffffff81111561035057600080fd5b61035c868287016101ea565b9150509250925092565b600081518084526020840193506020830160005b8281101561039857815186526020958601959091019060010161037a565b5093949350505050565b60006080820173ffffffffffffffffffffffffffffffffffffffff871683526080602084015280865180835260a08501915060208801925060005b8181101561041157835173ffffffffffffffffffffffffffffffffffffffff168352602093840193909201916001016103dd565b505083810360408501526104258187610366565b915050828103606084015261043a8185610366565b97965050505050505056fe608060405234801561001057600080fd5b506040516105c93803806105c983398101604081905261002f91610264565b836001600160a01b031663d7740ee16040518163ffffffff1660e01b81526004016020604051808303816000875af115801561006f573d6000803e3d6000fd5b505050506040513d601f19601f82011682018060405250810190610093919061036f565b6000819055604051637b09f30360e01b81526001600160a01b03861691637b09f303916100c7919087908690600401610388565b600060405180830381600087803b1580156100e157600080fd5b505af11580156100f5573d6000803e3d6000fd5b5050835161010c9250600191506020850190610116565b505050505061041c565b828054828255906000526020600020908101928215610151579160200282015b82811115610151578251825591602001919060010190610136565b5061015d929150610161565b5090565b5b8082111561015d5760008155600101610162565b6001600160a01b038116811461018b57600080fd5b50565b634e487b7160e01b600052604160045260246000fd5b604051601f8201601f191681016001600160401b03811182821017156101cc576101cc61018e565b604052919050565b60006001600160401b038211156101ed576101ed61018e565b5060051b60200190565b600082601f83011261020857600080fd5b815161021b610216826101d4565b6101a4565b8082825260208201915060208360051b86010192508583111561023d57600080fd5b602085015b8381101561025a578051835260209283019201610242565b5095945050505050565b6000806000806080858703121561027a57600080fd5b845161028581610176565b60208601519094506001600160401b038111156102a157600080fd5b8501601f810187136102b257600080fd5b80516102c0610216826101d4565b8082825260208201915060208360051b8501019250898311156102e257600080fd5b6020840193505b8284101561030d5783516102fc81610176565b8252602093840193909101906102e9565b6040890151909650925050506001600160401b0381111561032d57600080fd5b610339878288016101f7565b606087015190935090506001600160401b0381111561035757600080fd5b610363878288016101f7565b91505092959194509250565b60006020828403121561038157600080fd5b5051919050565b6000606082018583526060602084015280855180835260808501915060208701925060005b818110156103d45783516001600160a01b03168352602093840193909201916001016103ad565b505083810360408501528451808252602091820192509085019060005b8181101561040f5782518452602093840193909201916001016103f1565b5091979650505050505050565b61019e8061042b6000396000f3fe608060405234801561001057600080fd5b50600436106100675760003560e01c80633e0dc34e116100505780633e0dc34e146100a257806355c67628146100ab578063f89f27ed146100b857600080fd5b80631c0de0511461006c57806338fff2d014610090575b600080fd5b60408051600080825260208201819052918101919091526060015b60405180910390f35b6000545b604051908152602001610087565b61009460005481565b66038d7ea4c68000610094565b6100c06100cd565b6040516100879190610125565b6060600180548060200260200160405190810160405280929190818152602001828054801561011b57602002820191906000526020600020905b815481526020019060010190808311610107575b5050505050905090565b602080825282518282018190526000918401906040840190835b8181101561015d57835183526020938401939092019160010161013f565b50909594505050505056fea26469706673582212208263ec4ec001151dc4ddf8de16bd3bd182b409db8ddd3052a793aded797c56dd64736f6c634300081e0033a2646970667358221220b108d4f0e3109c7d72ed7ff943b08bc35c01f9716f63de3414ca057482ccecb864736f6c634300081e0033")]
    contract MockBalancerPoolFactory {
        constructor(address vault);
        event PoolCreated(address indexed pool);
        function createPool(
            address[] memory tokens,
            uint256[] memory weights,
            uint256[] memory balances,
        ) external returns (address pool);
    }
}

const POOL_INDEXER_PORT: u16 = 7778;
const POOL_INDEXER_HOST: &str = "http://127.0.0.1:7778";
const POOL_INDEXER_METRICS_PORT: u16 = 7779;

/// Builds a URL against the pool-indexer's Uniswap V3 API (all test fixtures
/// index `mainnet`), so call sites don't repeat the route prefix by hand.
fn v3_api(path: &str) -> String {
    format!("{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/{path}")
}
// The indexer has its own database (mirrors the per-network prod DB), migrated
// from `database/sql-pool-indexer` by the `migrations-pool-indexer` flyway step
// (docker-compose / setup-e2e.sh), separate from the shared autopilot DB.
const POOL_INDEXER_DB_URL: &str = "postgresql:///pool_indexer";

// sqrt(1) * 2^96 — valid starting price
const INITIAL_SQRT_PRICE: u128 = 1u128 << 96;

/// Typed shape of `GET /api/v1/{network}/uniswap/v3/pools`.
#[derive(Deserialize)]
struct PoolsListResponse {
    block_number: u64,
    pools: Vec<PoolEntry>,
    #[serde(default)]
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct PoolEntry {
    id: String,
}

/// Typed shape of `GET /api/v1/{network}/uniswap/v3/pools/{pool}/ticks`.
#[derive(Deserialize)]
struct TicksResponse {
    ticks: Vec<TickEntry>,
}

#[derive(Deserialize)]
struct TickEntry {}

/// Truncates the indexer's tables between tests. The schema itself is
/// provisioned by flyway (`migrations-pool-indexer`), so this just clears rows.
async fn clear_pool_indexer_tables(db: &PgPool) {
    sqlx::query(
        "TRUNCATE uniswap_v3_ticks, uniswap_v3_pool_states, uniswap_v3_pools, \
         balancer_v2_pool_tokens, balancer_v2_pools, pool_indexer_checkpoints",
    )
    .execute(db)
    .await
    .unwrap();
}

async fn seed_checkpoint(db: &PgPool, factory: Address, block: u64) {
    sqlx::query(
        "INSERT INTO pool_indexer_checkpoints (contract_address, block_number)
         VALUES ($1, $2)
         ON CONFLICT (contract_address) DO UPDATE SET block_number = EXCLUDED.block_number",
    )
    .bind(factory.as_slice())
    .bind(block.cast_signed())
    .execute(db)
    .await
    .unwrap();
}

fn pool_indexer_config(
    factories: impl IntoIterator<Item = Address>,
    metrics_port: u16,
) -> Configuration {
    Configuration {
        database: DatabaseConfig {
            url: POOL_INDEXER_DB_URL.parse().unwrap(),
            max_connections: NonZeroU32::new(5).unwrap(),
        },
        network: NetworkConfig {
            name: NetworkName::new("mainnet"),
            chain_id: 1,
            rpc_url: "http://127.0.0.1:8545".parse().unwrap(),
            uniswap_v3: Some(UniswapV3Config {
                factories: factories
                    .into_iter()
                    .map(|address| FactoryConfig {
                        address,
                        deploy_block: 0,
                    })
                    .collect(),
                chunk_size: 1000,
            }),
            poll_interval_secs: 1,
            use_latest: true,
            fetch_concurrency: 8,
            prefetch_concurrency: 50,
            balancer_v2: None,
        },
        api: ApiConfig {
            bind_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, POOL_INDEXER_PORT)),
        },
        metrics: MetricsConfig {
            bind_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, metrics_port)),
        },
    }
}

/// Spawns the pool-indexer task and waits for its `/health` endpoint to come
/// up.
async fn spawn_pool_indexer(config: Configuration) -> tokio::task::JoinHandle<()> {
    let handle = tokio::task::spawn(pool_indexer::run(config));
    wait_for_condition(TIMEOUT, || async {
        reqwest::get(format!("{POOL_INDEXER_HOST}/health"))
            .await
            .is_ok_and(|r| r.status().is_success())
    })
    .await
    .expect("pool-indexer API did not come up");
    handle
}

/// Runs `body` with a freshly-started pool-indexer. The indexer is spawned
/// before the closure runs, then `abort`ed and `await`ed after — so the port
/// is fully released before this returns, and a follow-up call can re-bind it.
async fn with_pool_indexer_at<F, Fut, T>(factories: &[Address], metrics_port: u16, body: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let handle =
        spawn_pool_indexer(pool_indexer_config(factories.iter().copied(), metrics_port)).await;
    let result = body().await;
    handle.abort();
    let _ = handle.await;
    result
}

/// Polls `/pools` until the indexer has reached `head` and surfaced at least
/// `min_pools` pools. `min_pools = 0` means "any state is fine, just check
/// block_number".
async fn wait_for_indexer(head: u64, min_pools: usize) {
    wait_for_condition(TIMEOUT, || async {
        let resp = reqwest::get(v3_api("pools")).await.ok()?;
        let body: PoolsListResponse = resp.json().await.ok()?;
        Some(body.block_number >= head && body.pools.len() >= min_pools)
    })
    .await
    .expect("indexer did not reach target state");
}

/// Samples `(pool_count, sqrt_price_x96, tick, liquidity)` for a single pool.
async fn snapshot_pool_state(db: &PgPool, pool_addr: Address) -> (i64, String, i32, String) {
    let pool_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools")
        .fetch_one(db)
        .await
        .unwrap();
    let (sqrt_price, tick, liquidity): (String, i32, String) = sqlx::query_as(
        "SELECT sqrt_price_x96::TEXT, tick, liquidity::TEXT
         FROM uniswap_v3_pool_states
         WHERE pool_address = $1",
    )
    .bind(pool_addr.as_slice())
    .fetch_one(db)
    .await
    .unwrap();
    (pool_count, sqrt_price, tick, liquidity)
}

/// Create + initialise a single pool inside an already-deployed factory.
/// fee must be unique within the factory for token0/token1 ([1u8;20],[2u8;20]).
async fn create_pool(
    factory: &MockUniswapV3Factory::MockUniswapV3FactoryInstance<impl Provider>,
    fee: u32,
) -> Address {
    let provider = factory.provider();
    let token0 = Address::repeat_byte(1);
    let token1 = Address::repeat_byte(2);

    factory
        .createPool(token0, token1, U24::from(fee))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    let block = provider.get_block_number().await.unwrap();
    let logs = provider
        .get_logs(
            &alloy::rpc::types::Filter::new()
                .from_block(block)
                .to_block(block)
                .event_signature(MockUniswapV3Factory::PoolCreated::SIGNATURE_HASH),
        )
        .await
        .unwrap();
    let pool_addr = MockUniswapV3Factory::PoolCreated::decode_log(&logs[0].inner)
        .unwrap()
        .data
        .pool;

    let pool = MockUniswapV3Pool::MockUniswapV3PoolInstance::new(pool_addr, provider);

    pool.initialize(U160::from(INITIAL_SQRT_PRICE))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    pool.mockMint(
        token0,
        I24::try_from(-100i32).unwrap(),
        I24::try_from(100i32).unwrap(),
        1_000_000u128,
    )
    .send()
    .await
    .unwrap()
    .watch()
    .await
    .unwrap();

    pool_addr
}

/// Deploy mock V3 contracts and set up a pool with liquidity.
async fn deploy_univ3(
    web3: &Web3,
) -> (
    MockUniswapV3Factory::MockUniswapV3FactoryInstance<alloy::providers::DynProvider>,
    Address,
) {
    let provider = web3.provider.clone().erased();

    let factory = MockUniswapV3Factory::deploy(provider.clone())
        .await
        .unwrap();
    let pool_addr = create_pool(&factory, 500).await;

    (factory, pool_addr)
}

/// Parse the `pool_indexer_api_requests` Prometheus counter for a given
/// route from the indexer's /metrics endpoint.
async fn api_requests_counter(metrics_port: u16, route: &'static str) -> u64 {
    let body = reqwest::get(format!("http://127.0.0.1:{metrics_port}/metrics"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let needle = format!("pool_indexer_api_requests{{route=\"{route}\"");
    for line in body.lines() {
        if line.starts_with('#') {
            continue;
        }
        if let Some(idx) = line.find(&needle) {
            // pool_indexer_api_requests{route="...",status="200"} 3
            let after = line[idx + needle.len()..].trim();
            if let Some(value) = after.split_whitespace().last() {
                return value.parse().unwrap_or(0);
            }
        }
    }
    0
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_driver_integration() {
    run_test(driver_integration).await;
}

/// Asserts (via the indexer's own request counters) that a driver pointed at
/// `pool-indexer-url` fetched pools AND their ticks. Ticks is the stronger
/// signal — only hit after `UniswapV3PoolFetcher::new` sees a non-empty set.
async fn driver_integration(web3: Web3) {
    const POOLS_ROUTE: &str = "/api/v1/{network}/uniswap/v3/pools";
    const POOLS_BY_IDS_ROUTE: &str = "/api/v1/{network}/uniswap/v3/pools/by-ids";
    const TICKS_ROUTE: &str = "/api/v1/{network}/uniswap/v3/pools/ticks";

    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;

    let (factory, pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    with_pool_indexer_at(&[factory_addr], POOL_INDEXER_METRICS_PORT, || async {
        // Without the min_pools=1 gate the driver could race against an empty
        // set and skip the ticks fetch this test asserts on.
        wait_for_indexer(head, 1).await;

        // Mock tokens have no real `decimals()`; backfill plausible values so
        // the driver's `pools_tokens_have_decimals` filter doesn't drop them.
        sqlx::query(
            "UPDATE uniswap_v3_pools SET token0_decimals = 18, token1_decimals = 6 WHERE address \
             = $1",
        )
        .bind(pool_addr.as_slice())
        .execute(&db)
        .await
        .unwrap();

        // Baseline AFTER warm-up polling so bumps below are driver-attributable.
        let baseline_pools = api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_ROUTE).await;
        let baseline_pools_by_ids =
            api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_BY_IDS_ROUTE).await;
        let baseline_ticks = api_requests_counter(POOL_INDEXER_METRICS_PORT, TICKS_ROUTE).await;

        let baseline_solver = colocation::start_baseline_solver(
            "test_solver".into(),
            solver.clone(),
            *onchain.contracts().weth.address(),
            vec![],
            1,
            true,
        )
        .await;

        // Router address only used at settlement time; any 20-byte value works.
        let config_override = format!(
            r#"
[[liquidity.uniswap-v3]]
router = "0x000000000000000000000000000000000000dEaD"
indexer-url = "{POOL_INDEXER_HOST}"
max-pools-to-initialize = 10
"#
        );
        let driver_handle = colocation::start_driver_with_config_override(
            onchain.contracts(),
            vec![baseline_solver],
            colocation::LiquidityProvider::UniswapV2,
            false,
            Some(&config_override),
        );

        wait_for_condition(TIMEOUT, || async {
            let pools = api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_ROUTE).await;
            let pools_by_ids =
                api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_BY_IDS_ROUTE).await;
            let ticks = api_requests_counter(POOL_INDEXER_METRICS_PORT, TICKS_ROUTE).await;
            pools > baseline_pools && pools_by_ids > baseline_pools_by_ids && ticks > baseline_ticks
        })
        .await
        .expect("driver did not complete pool + tick fetch from pool-indexer within timeout");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools")
            .fetch_one(&db)
            .await
            .unwrap();
        assert!(count > 0, "expected pools persisted to DB");

        driver_handle.abort();
    })
    .await;
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_checkpoint_resume() {
    run_test(checkpoint_resume).await;
}

/// Re-running the indexer over the same DB must merge into existing rows
/// (no duplicates) and leave per-pool state untouched. Asserts that pool
/// count, sqrt_price / tick / liquidity, and the checkpoint all survive a
/// stop+start.
async fn checkpoint_resume(web3: Web3) {
    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    let before = indexer_pass_snapshot(factory_addr, head, &db, pool_addr).await;
    let after = indexer_pass_snapshot(factory_addr, head, &db, pool_addr).await;
    assert_eq!(before, after, "indexer state changed across restart");

    let checkpoint: i64 = sqlx::query_scalar(
        "SELECT block_number FROM pool_indexer_checkpoints WHERE contract_address = $1",
    )
    .bind(factory_addr.as_slice())
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(
        checkpoint as u64 >= head,
        "checkpoint did not advance to head"
    );
}

/// One full indexer lifecycle (start → wait for head → snapshot → stop).
/// Two calls in sequence over the same DB are how `checkpoint_resume`
/// proves the restart preserves state.
async fn indexer_pass_snapshot(
    factory_addr: Address,
    head: u64,
    db: &PgPool,
    pool_addr: Address,
) -> (i64, String, i32, String) {
    with_pool_indexer_at(&[factory_addr], POOL_INDEXER_METRICS_PORT, || async {
        wait_for_indexer(head, 0).await;
        snapshot_pool_state(db, pool_addr).await
    })
    .await
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_api_errors() {
    run_test(api_errors).await;
}

/// Input-validation surface: an unparseable pool address must come back as
/// 400, a valid-but-unknown address must come back as 200 with empty ticks.
/// Lets callers distinguish "garbage input" from "no data yet".
async fn api_errors(web3: Web3) {
    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, _pool) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    with_pool_indexer_at(&[factory_addr], POOL_INDEXER_METRICS_PORT, || async {
        wait_for_indexer(head, 0).await;
        invalid_address_returns_400().await;
        unknown_address_returns_empty_ticks().await;
    })
    .await;
}

async fn invalid_address_returns_400() {
    let status = reqwest::get(v3_api("pools/not-an-address/ticks"))
        .await
        .unwrap()
        .status();
    assert_eq!(u16::from(status), 400, "expected 400 for invalid address");
}

async fn unknown_address_returns_empty_ticks() {
    let unknown = Address::repeat_byte(0xAB);
    let resp: TicksResponse = reqwest::get(v3_api(&format!("pools/{unknown:?}/ticks")))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        resp.ticks.is_empty(),
        "expected empty ticks for unknown pool"
    );
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_pagination() {
    run_test(pagination).await;
}

/// Cursor pagination: stepping through /pools with limit=1 must traverse
/// every pool exactly once. Three pools is the smallest set that exercises
/// a mid-stream cursor and the `next_cursor = null` terminator.
async fn pagination(web3: Web3) {
    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, _pool1) = deploy_univ3(&web3).await;
    let _pool2 = create_pool(&factory, 3000).await;
    let _pool3 = create_pool(&factory, 10000).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    with_pool_indexer_at(&[factory_addr], POOL_INDEXER_METRICS_PORT, || async {
        wait_for_indexer(head, 3).await;

        let mut all_ids: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let url = match &cursor {
                None => v3_api("pools?limit=1"),
                Some(c) => v3_api(&format!("pools?limit=1&after={c}")),
            };
            let resp: PoolsListResponse = reqwest::get(&url).await.unwrap().json().await.unwrap();
            if resp.pools.is_empty() {
                break;
            }
            for p in resp.pools {
                all_ids.push(p.id);
            }
            cursor = resp.next_cursor;
            if cursor.is_none() {
                break;
            }
        }

        let db_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(
            i64::try_from(all_ids.len()).unwrap(),
            db_count,
            "paginated count doesn't match DB"
        );
        assert!(
            db_count >= 3,
            "expected at least 3 pools to exercise pagination"
        );
        let unique: std::collections::HashSet<_> = all_ids.iter().collect();
        assert_eq!(
            unique.len(),
            all_ids.len(),
            "pagination returned duplicates"
        );
    })
    .await;
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_bootstrap_idempotent() {
    run_test(bootstrap_idempotent).await;
}

/// `--bootstrap-only` on an already-caught-up DB must be a fast no-op: the
/// catch-up loop sees the checkpoint is already at the head and returns without
/// binding any ports, like a bootstrap initContainer re-run on a pod restart.
async fn bootstrap_idempotent(web3: Web3) {
    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    // A checkpoint at the head means the DB is already caught up, so the
    // catch-up loop finds nothing to do. No on-chain factory is needed; the RPC
    // only serves the chain_id check and reading the finalized head.
    let factory = Address::repeat_byte(0x11);
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory, head).await;

    tokio::time::timeout(
        TIMEOUT,
        pool_indexer::bootstrap(pool_indexer_config([factory], POOL_INDEXER_METRICS_PORT)),
    )
    .await
    .expect("bootstrap-only did not exit on an already-seeded DB");

    let checkpoint: i64 = sqlx::query_scalar(
        "SELECT block_number FROM pool_indexer_checkpoints WHERE contract_address = $1",
    )
    .bind(factory.as_slice())
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        checkpoint,
        head.cast_signed(),
        "bootstrap mutated an already-seeded checkpoint"
    );
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_onchain_cold_seed() {
    run_test(onchain_cold_seed).await;
}

/// Cold bootstrap with no checkpoint: the indexer discovers the pool and
/// reconstructs its state by replaying on-chain events from the factory's
/// deploy block.
async fn onchain_cold_seed(web3: Web3) {
    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();

    // No seed_checkpoint: bootstrap must cold-seed on-chain (deploy_block = 0).
    with_pool_indexer_at(&[factory_addr], POOL_INDEXER_METRICS_PORT, || async {
        wait_for_indexer(head, 1).await;

        // Pool discovered + state rebuilt from the Initialize (sqrt_price) and
        // Mint (liquidity) events alone; no pre-seeded checkpoint.
        let (count, sqrt_price, _tick, liquidity) = snapshot_pool_state(&db, pool_addr).await;
        assert_eq!(
            count, 1,
            "the pool should be discovered by the on-chain scan"
        );
        assert_eq!(
            sqrt_price,
            INITIAL_SQRT_PRICE.to_string(),
            "sqrt_price reconstructed from the Initialize event"
        );
        assert_eq!(
            liquidity, "1000000",
            "liquidity reconstructed from the Mint event"
        );
    })
    .await;
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_two_factories() {
    run_test(two_factories).await;
}

/// Two factories on one network are served as a single union: each factory's
/// pools show up in `/pools`, attributed to that factory.
async fn two_factories(web3: Web3) {
    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    // One pool per factory, at distinct addresses.
    let (factory_a, pool_a) = deploy_univ3(&web3).await;
    let (factory_b, pool_b) = deploy_univ3(&web3).await;
    assert_ne!(pool_a, pool_b, "each factory deploys its own pool");
    let factory_a_addr = *factory_a.address();
    let factory_b_addr = *factory_b.address();
    let head = web3.provider.get_block_number().await.unwrap();

    seed_checkpoint(&db, factory_a_addr, 0).await;
    seed_checkpoint(&db, factory_b_addr, 0).await;

    with_pool_indexer_at(
        &[factory_a_addr, factory_b_addr],
        POOL_INDEXER_METRICS_PORT,
        || async {
            wait_for_indexer(head, 2).await;

            let resp: PoolsListResponse = reqwest::get(v3_api("pools"))
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(
                resp.pools.len(),
                2,
                "both factories' pools should be served as one union"
            );

            // Each pool is owned by its factory.
            for (factory, pool) in [(factory_a_addr, pool_a), (factory_b_addr, pool_b)] {
                let owned: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM uniswap_v3_pools WHERE factory = $1 AND address = $2",
                )
                .bind(factory.as_slice())
                .bind(pool.as_slice())
                .fetch_one(&db)
                .await
                .unwrap();
                assert_eq!(
                    owned, 1,
                    "pool {pool:#x} should be owned by factory {factory:#x}"
                );
            }

            let (_, _, _, liq_before) = snapshot_pool_state(&db, pool_a).await;
            let pool_a_contract = MockUniswapV3Pool::MockUniswapV3PoolInstance::new(
                pool_a,
                web3.provider.clone().erased(),
            );
            pool_a_contract
                .mockMint(
                    Address::repeat_byte(1),
                    I24::try_from(-200i32).unwrap(),
                    I24::try_from(200i32).unwrap(),
                    1_000_000u128,
                )
                .send()
                .await
                .unwrap()
                .watch()
                .await
                .unwrap();
            let new_head = web3.provider.get_block_number().await.unwrap();
            wait_for_indexer(new_head, 2).await;

            let (_, _, _, liq_after) = snapshot_pool_state(&db, pool_a).await;
            assert!(
                liq_after.parse::<u128>().unwrap() > liq_before.parse::<u128>().unwrap(),
                "pool A's mint was indexed"
            );
            let (_, _, _, liq_b) = snapshot_pool_state(&db, pool_b).await;
            assert_eq!(liq_b, "1000000", "pool B unchanged");
        },
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_min_envelope() {
    run_test(min_envelope).await;
}

/// The envelope block is the MIN across the configured factories' checkpoints,
/// so the union is never advertised as fresher than its slowest factory. The
/// MIN is scoped to configured factories: a decommissioned factory's leftover
/// checkpoint (nothing deletes it) must not pin the envelope.
async fn min_envelope(_web3: Web3) {
    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let factory_ahead = Address::from([0x11; 20]);
    let factory_behind = Address::from([0x22; 20]);
    // A leftover checkpoint from a factory no longer in config, far behind. If
    // the envelope weren't scoped to configured factories it would pin here.
    let decommissioned = Address::from([0x33; 20]);
    // Above the head, so the live loops stay put at the seeded blocks.
    seed_checkpoint(&db, factory_ahead, 1_000_000).await;
    seed_checkpoint(&db, factory_behind, 999_999).await;
    seed_checkpoint(&db, decommissioned, 1).await;

    with_pool_indexer_at(
        &[factory_ahead, factory_behind],
        POOL_INDEXER_METRICS_PORT,
        || async {
            let resp: PoolsListResponse = reqwest::get(v3_api("pools?limit=1"))
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(
                resp.block_number, 999_999,
                "envelope should be the MIN across configured factories, ignoring the \
                 decommissioned one"
            );
        },
    )
    .await;
}

/// Pool-indexer config indexing the given balancer factories against `vault`.
fn balancer_pool_indexer_config(
    vault: Address,
    weighted: Vec<Address>,
    stable: Vec<Address>,
    metrics_port: u16,
) -> Configuration {
    let factory = |address| FactoryConfig {
        address,
        deploy_block: 0,
    };
    Configuration {
        database: DatabaseConfig {
            url: POOL_INDEXER_DB_URL.parse().unwrap(),
            max_connections: NonZeroU32::new(5).unwrap(),
        },
        network: NetworkConfig {
            name: NetworkName::new("mainnet"),
            chain_id: 1,
            rpc_url: "http://127.0.0.1:8545".parse().unwrap(),
            uniswap_v3: None,
            balancer_v2: Some(BalancerV2Config {
                vault,
                chunk_size: 1000,
                weighted: weighted.into_iter().map(factory).collect(),
                weighted_v3plus: vec![],
                stable: stable.into_iter().map(factory).collect(),
                liquidity_bootstrapping: vec![],
                composable_stable: vec![],
            }),
            poll_interval_secs: 1,
            use_latest: true,
            fetch_concurrency: 8,
            prefetch_concurrency: 50,
        },
        api: ApiConfig {
            bind_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, POOL_INDEXER_PORT)),
        },
        metrics: MetricsConfig {
            bind_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, metrics_port)),
        },
    }
}

fn balancer_api(path: &str) -> String {
    format!("{POOL_INDEXER_HOST}/api/v1/mainnet/balancer/v2/{path}")
}

#[derive(Debug, Deserialize)]
struct BalancerPoolsListResponse {
    block_number: u64,
    pools: Vec<BalancerPoolResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BalancerPoolResponse {
    pool_type: String,
    address: Address,
    factory: Address,
    swap_enabled: bool,
    tokens: Vec<BalancerTokenResponse>,
}

#[derive(Debug, Deserialize)]
struct BalancerTokenResponse {
    address: Address,
    decimals: u8,
    #[serde(default)]
    weight: Option<String>,
}

/// Creates a mock balancer pool and returns its address (from `PoolCreated`).
async fn create_balancer_pool(
    factory: &MockBalancerPoolFactory::MockBalancerPoolFactoryInstance<impl Provider>,
    tokens: Vec<Address>,
    weights: Vec<U256>,
    balances: Vec<U256>,
) -> Address {
    let provider = factory.provider();
    factory
        .createPool(tokens, weights, balances)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();
    let block = provider.get_block_number().await.unwrap();
    let logs = provider
        .get_logs(
            &alloy::rpc::types::Filter::new()
                .from_block(block)
                .to_block(block)
                .event_signature(MockBalancerPoolFactory::PoolCreated::SIGNATURE_HASH),
        )
        .await
        .unwrap();
    MockBalancerPoolFactory::PoolCreated::decode_log(&logs[0].inner)
        .unwrap()
        .data
        .pool
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_balancer_discovery() {
    run_test(balancer_discovery).await;
}

/// Asserts the indexer discovers a weighted + a stable pool and serves both
/// with the right type, token order, decimals, and weights (weighted only).
async fn balancer_discovery(web3: Web3) {
    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;
    let provider = web3.provider.clone().erased();

    let vault = MockBalancerVault::deploy(provider.clone()).await.unwrap();
    let weighted_factory = MockBalancerPoolFactory::deploy(provider.clone(), *vault.address())
        .await
        .unwrap();
    let stable_factory = MockBalancerPoolFactory::deploy(provider.clone(), *vault.address())
        .await
        .unwrap();
    let token18 = MockToken::deploy(provider.clone(), 18u8).await.unwrap();
    let token6 = MockToken::deploy(provider.clone(), 6u8).await.unwrap();

    // Weighted pool with 60/40 weights; stable pool carries no weights.
    let weighted_tokens = vec![*token18.address(), *token6.address()];
    let weighted_pool = create_balancer_pool(
        &weighted_factory,
        weighted_tokens.clone(),
        vec![
            U256::from(600_000_000_000_000_000u128),
            U256::from(400_000_000_000_000_000u128),
        ],
        vec![U256::ZERO, U256::ZERO],
    )
    .await;
    let stable_tokens = vec![*token6.address(), *token18.address()];
    let stable_pool = create_balancer_pool(
        &stable_factory,
        stable_tokens.clone(),
        vec![],
        vec![U256::ZERO, U256::ZERO],
    )
    .await;

    seed_checkpoint(&db, *weighted_factory.address(), 0).await;
    seed_checkpoint(&db, *stable_factory.address(), 0).await;
    let head = provider.get_block_number().await.unwrap();

    let config = balancer_pool_indexer_config(
        *vault.address(),
        vec![*weighted_factory.address()],
        vec![*stable_factory.address()],
        POOL_INDEXER_METRICS_PORT,
    );
    let handle = spawn_pool_indexer(config).await;

    wait_for_condition(TIMEOUT, || async {
        let body: BalancerPoolsListResponse = reqwest::get(balancer_api("pools"))
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        Some(body.block_number >= head && body.pools.len() >= 2)
    })
    .await
    .expect("indexer did not serve both balancer pools");

    let body: BalancerPoolsListResponse = reqwest::get(balancer_api("pools"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let by_addr = |addr: Address| body.pools.iter().find(|p| p.address == addr).unwrap();

    // Weighted: type + tokens (getPoolTokens order) + decimals + weights.
    let w = by_addr(weighted_pool);
    assert_eq!(w.pool_type, "Weighted");
    assert_eq!(w.factory, *weighted_factory.address());
    assert!(w.swap_enabled);
    assert_eq!(
        w.tokens.iter().map(|t| t.address).collect::<Vec<_>>(),
        weighted_tokens
    );
    assert_eq!(w.tokens[0].decimals, 18);
    assert_eq!(w.tokens[1].decimals, 6);
    assert_eq!(w.tokens[0].weight.as_deref(), Some("0.6"));
    assert_eq!(w.tokens[1].weight.as_deref(), Some("0.4"));

    // Stable: type + tokens (order) + decimals; no weights.
    let s = by_addr(stable_pool);
    assert_eq!(s.pool_type, "Stable");
    assert_eq!(s.factory, *stable_factory.address());
    assert_eq!(
        s.tokens.iter().map(|t| t.address).collect::<Vec<_>>(),
        stable_tokens
    );
    assert_eq!(s.tokens[0].decimals, 6);
    assert_eq!(s.tokens[1].decimals, 18);
    assert!(s.tokens.iter().all(|t| t.weight.is_none()));

    handle.abort();
    let _ = handle.await;
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_balancer_driver_integration() {
    run_test(balancer_driver_integration).await;
}

/// Balancer analog of `driver_integration`: asserts (via the indexer's request
/// counter) that a driver with a `[[liquidity.balancer-v2]]` + `indexer-url`
/// source cold-reads the pool registry from the indexer at startup. Balancer
/// pool state is on-chain, so `/balancer/v2/pools` is the single cold-read
/// route (uni-v3 additionally serves ticks).
async fn balancer_driver_integration(web3: Web3) {
    const POOLS_ROUTE: &str = "/api/v1/{network}/balancer/v2/pools";

    let db = PgPool::connect(POOL_INDEXER_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let weth = *onchain.contracts().weth.address();

    // WETH/token weighted pool via the mock factory the indexer scans and the
    // driver's balancer config points at.
    let provider = web3.provider.clone().erased();
    let vault = MockBalancerVault::deploy(provider.clone()).await.unwrap();
    let factory = MockBalancerPoolFactory::deploy(provider.clone(), *vault.address())
        .await
        .unwrap();
    let token = MockToken::deploy(provider.clone(), 6u8).await.unwrap();
    create_balancer_pool(
        &factory,
        vec![weth, *token.address()],
        vec![
            U256::from(500_000_000_000_000_000u128),
            U256::from(500_000_000_000_000_000u128),
        ],
        vec![
            U256::from(100u128) * U256::from(10).pow(U256::from(18)), // 100 WETH
            U256::from(300_000u128) * U256::from(10).pow(U256::from(6)), // 300k token
        ],
    )
    .await;

    let vault_addr = *vault.address();
    let factory_addr = *factory.address();
    seed_checkpoint(&db, factory_addr, 0).await;
    let head = provider.get_block_number().await.unwrap();

    let config = balancer_pool_indexer_config(
        vault_addr,
        vec![factory_addr],
        vec![],
        POOL_INDEXER_METRICS_PORT,
    );
    let indexer = spawn_pool_indexer(config).await;

    wait_for_condition(TIMEOUT, || async {
        let body: BalancerPoolsListResponse = reqwest::get(balancer_api("pools"))
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        Some(body.block_number >= head && !body.pools.is_empty())
    })
    .await
    .expect("indexer did not discover the balancer pool");

    // Baseline AFTER warm-up so the bump below is driver-attributable.
    let baseline_pools = api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_ROUTE).await;

    let baseline_solver = colocation::start_baseline_solver(
        "test_solver".into(),
        solver.clone(),
        weth,
        vec![],
        1,
        true,
    )
    .await;

    let config_override = format!(
        r#"
[[liquidity.balancer-v2]]
vault = "{vault_addr:?}"
weighted = ["{factory_addr:?}"]
indexer-url = "{POOL_INDEXER_HOST}"
"#
    );
    let driver_handle = colocation::start_driver_with_config_override(
        onchain.contracts(),
        vec![baseline_solver],
        colocation::LiquidityProvider::UniswapV2,
        false,
        Some(&config_override),
    );

    // The driver seeds its balancer registry from the indexer in the
    // background, bumping the counter.
    wait_for_condition(TIMEOUT, || async {
        api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_ROUTE).await > baseline_pools
    })
    .await
    .expect("driver did not cold-read balancer pools from the pool-indexer within timeout");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM balancer_v2_pools")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(count > 0, "expected balancer pools persisted to DB");

    driver_handle.abort();
    indexer.abort();
    let _ = indexer.await;
}

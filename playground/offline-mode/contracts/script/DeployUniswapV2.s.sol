// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";
import {DeploymentUtils} from "./Utils.sol";

// We'll use interfaces since Uniswap V2 uses Solidity 0.5.16
interface IUniswapV2Factory {
    function createPair(address tokenA, address tokenB) external returns (address pair);
    function getPair(address tokenA, address tokenB) external view returns (address pair);
}

interface IUniswapV2Router02 {
    function factory() external pure returns (address);
    function WETH() external pure returns (address);
    function addLiquidity(
        address tokenA,
        address tokenB,
        uint amountADesired,
        uint amountBDesired,
        uint amountAMin,
        uint amountBMin,
        address to,
        uint deadline
    ) external returns (uint amountA, uint amountB, uint liquidity);
}

/// @title DeployUniswapV2
/// @notice Deploy Uniswap V2 Factory, Router, and create token pairs using CREATE2
contract DeployUniswapV2 is Script {
    // Deterministic salts for CREATE2
    bytes32 constant FACTORY_SALT = keccak256("uniswap-v2-factory");
    bytes32 constant ROUTER_SALT = keccak256("uniswap-v2-router");
    function run() external {
        // Load deployer private key
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);

        // Load token addresses
        address weth = vm.envAddress("WETH_ADDRESS");
        address usdc = vm.envAddress("USDC_ADDRESS");
        address dai = vm.envAddress("DAI_ADDRESS");
        address usdt = vm.envAddress("USDT_ADDRESS");
        address gno = vm.envAddress("GNO_ADDRESS");

        console.log("===========================================");
        console.log("DEPLOYING UNISWAP V2");
        console.log("===========================================");
        console.log("Deployer:", deployer);
        console.log("Chain ID:", block.chainid);
        console.log("");
        console.log("Token addresses:");
        console.log("  WETH:", weth);
        console.log("  USDC:", usdc);
        console.log("  DAI:", dai);
        console.log("  USDT:", usdt);
        console.log("  GNO:", gno);
        console.log("");

        vm.startBroadcast(deployerPrivateKey);

        // Deploy Factory using CREATE2 with official precompiled bytecode from npm package
        console.log("Deploying UniswapV2Factory with CREATE2...");
        string memory factoryJson = vm.readFile("node_modules/@uniswap/v2-core/build/UniswapV2Factory.json");
        bytes memory factoryCreationCode = vm.parseJsonBytes(factoryJson, ".bytecode");
        bytes memory factoryBytecode = abi.encodePacked(
            factoryCreationCode,
            abi.encode(deployer) // feeToSetter parameter
        );
        address factory = DeploymentUtils.deployWithCreate2(factoryBytecode, FACTORY_SALT);
        require(factory != address(0), "Factory deployment failed");
        console.log("  Factory deployed at:", factory);

        // Deploy Router using CREATE2 with official precompiled bytecode from npm package
        console.log("");
        console.log("Deploying UniswapV2Router02 with CREATE2...");
        string memory routerJson = vm.readFile("node_modules/@uniswap/v2-periphery/build/UniswapV2Router02.json");
        bytes memory routerCreationCode = vm.parseJsonBytes(routerJson, ".bytecode");
        bytes memory routerBytecode = abi.encodePacked(
            routerCreationCode,
            abi.encode(factory, weth)
        );
        address router = DeploymentUtils.deployWithCreate2(routerBytecode, ROUTER_SALT);
        require(router != address(0), "Router deployment failed");
        console.log("  Router deployed at:", router);

        // Create pairs
        console.log("");
        console.log("Creating trading pairs...");

        IUniswapV2Factory factoryContract = IUniswapV2Factory(factory);

        // WETH pairs (existing)
        console.log("  Creating WETH-USDC pair...");
        address wethUsdcPair = factoryContract.createPair(weth, usdc);
        console.log("    WETH-USDC pair:", wethUsdcPair);

        console.log("  Creating WETH-DAI pair...");
        address wethDaiPair = factoryContract.createPair(weth, dai);
        console.log("    WETH-DAI pair:", wethDaiPair);

        console.log("  Creating WETH-USDT pair...");
        address wethUsdtPair = factoryContract.createPair(weth, usdt);
        console.log("    WETH-USDT pair:", wethUsdtPair);

        console.log("  Creating WETH-GNO pair...");
        address wethGnoPair = factoryContract.createPair(weth, gno);
        console.log("    WETH-GNO pair:", wethGnoPair);

        // USDC pairs
        console.log("  Creating USDC-DAI pair...");
        address usdcDaiPair = factoryContract.createPair(usdc, dai);
        console.log("    USDC-DAI pair:", usdcDaiPair);

        console.log("  Creating USDC-USDT pair...");
        address usdcUsdtPair = factoryContract.createPair(usdc, usdt);
        console.log("    USDC-USDT pair:", usdcUsdtPair);

        console.log("  Creating USDC-GNO pair...");
        address usdcGnoPair = factoryContract.createPair(usdc, gno);
        console.log("    USDC-GNO pair:", usdcGnoPair);

        // DAI pairs
        console.log("  Creating DAI-USDT pair...");
        address daiUsdtPair = factoryContract.createPair(dai, usdt);
        console.log("    DAI-USDT pair:", daiUsdtPair);

        console.log("  Creating DAI-GNO pair...");
        address daiGnoPair = factoryContract.createPair(dai, gno);
        console.log("    DAI-GNO pair:", daiGnoPair);

        // USDT-GNO pair
        console.log("  Creating USDT-GNO pair...");
        address usdtGnoPair = factoryContract.createPair(usdt, gno);
        console.log("    USDT-GNO pair:", usdtGnoPair);

        vm.stopBroadcast();

        console.log("");
        console.log("===========================================");
        console.log("DEPLOYMENT SUMMARY");
        console.log("===========================================");
        console.log("Factory:", factory);
        console.log("Router:", router);
        console.log("");
        console.log("Pairs:");
        console.log("  WETH-USDC:", wethUsdcPair);
        console.log("  WETH-DAI:", wethDaiPair);
        console.log("  WETH-USDT:", wethUsdtPair);
        console.log("  WETH-GNO:", wethGnoPair);
        console.log("  USDC-DAI:", usdcDaiPair);
        console.log("  USDC-USDT:", usdcUsdtPair);
        console.log("  USDC-GNO:", usdcGnoPair);
        console.log("  DAI-USDT:", daiUsdtPair);
        console.log("  DAI-GNO:", daiGnoPair);
        console.log("  USDT-GNO:", usdtGnoPair);
        console.log("===========================================");
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

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
/// @notice Deploy Uniswap V2 Factory, Router, and create token pairs
contract DeployUniswapV2 is Script {
    function run() external {
        // Load deployer private key
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);

        // Load token addresses
        address weth = vm.envAddress("WETH_ADDRESS");
        address usdc = vm.envAddress("USDC_ADDRESS");
        address dai = vm.envAddress("DAI_ADDRESS");

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
        console.log("");

        vm.startBroadcast(deployerPrivateKey);

        // Deploy Factory using compiled bytecode from Solidity 0.5.16
        console.log("Deploying UniswapV2Factory...");
        address factory = deployCode(
            "contracts/out-uniswap-v2/UniswapV2Factory.sol/UniswapV2Factory.json",
            abi.encode(deployer) // feeToSetter parameter
        );
        require(factory != address(0), "Factory deployment failed");
        console.log("  Factory deployed at:", factory);

        // Deploy Router using compiled bytecode from Solidity 0.6.6
        console.log("");
        console.log("Deploying UniswapV2Router02...");
        address router = deployCode(
            "contracts/out-uniswap-v2-periphery/UniswapV2Router02.sol/UniswapV2Router02.json",
            abi.encode(factory, weth)
        );
        require(router != address(0), "Router deployment failed");
        console.log("  Router deployed at:", router);

        // Create pairs
        console.log("");
        console.log("Creating trading pairs...");
        
        IUniswapV2Factory factoryContract = IUniswapV2Factory(factory);
        
        // WETH-USDC pair
        console.log("  Creating WETH-USDC pair...");
        address wethUsdcPair = factoryContract.createPair(weth, usdc);
        console.log("    WETH-USDC pair:", wethUsdcPair);
        
        // WETH-DAI pair
        console.log("  Creating WETH-DAI pair...");
        address wethDaiPair = factoryContract.createPair(weth, dai);
        console.log("    WETH-DAI pair:", wethDaiPair);
        
        // USDC-DAI pair
        console.log("  Creating USDC-DAI pair...");
        address usdcDaiPair = factoryContract.createPair(usdc, dai);
        console.log("    USDC-DAI pair:", usdcDaiPair);

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
        console.log("  USDC-DAI:", usdcDaiPair);
        console.log("===========================================");
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

interface IERC20 {
    function transfer(address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

interface IUniswapV2Pair {
    function mint(address to) external returns (uint liquidity);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
}

interface IUniswapV2Factory {
    function getPair(address tokenA, address tokenB) external view returns (address pair);
}

interface IWETH {
    function deposit() external payable;
}

/// @title AddLiquidityDirect
/// @notice Add initial liquidity to Uniswap V2 pairs using direct transfer + mint
contract AddLiquidityDirect is Script {
    function run() external {
        // Load deployer private key
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);

        // Load token addresses
        address weth = vm.envAddress("WETH_ADDRESS");
        address usdc = vm.envAddress("USDC_ADDRESS");
        address dai = vm.envAddress("DAI_ADDRESS");
        
        // Load Uniswap factory - REQUIRED for pair lookup
        address factory = vm.envAddress("UNISWAP_FACTORY");
        
        console.log("===========================================");
        console.log("ADDING LIQUIDITY TO UNISWAP V2 PAIRS");
        console.log("===========================================");
        console.log("Deployer:", deployer);
        console.log("Factory:", factory);
        console.log("");
        console.log("Tokens:");
        console.log("  WETH:", weth);
        console.log("  USDC:", usdc);
        console.log("  DAI:", dai);
        console.log("");

        vm.startBroadcast(deployerPrivateKey);

        // Liquidity amounts (using realistic ratios)
        // WETH = $2000, USDC = $1, DAI = $1
        uint256 wethForUsdc = 10 ether;      // 10 WETH
        uint256 usdcAmount = 20_000 * 1e6;   // 20,000 USDC (6 decimals)
        
        uint256 wethForDai = 10 ether;       // 10 WETH
        uint256 daiAmount = 20_000 * 1e18;   // 20,000 DAI (18 decimals)
        
        uint256 usdcForDai = 10_000 * 1e6;   // 10,000 USDC
        uint256 daiForUsdc = 10_000 * 1e18;  // 10,000 DAI

        // Get pair addresses from factory
        address wethUsdcPair = IUniswapV2Factory(factory).getPair(weth, usdc);
        require(wethUsdcPair != address(0), "WETH-USDC pair not found!");
        
        address wethDaiPair = IUniswapV2Factory(factory).getPair(weth, dai);
        require(wethDaiPair != address(0), "WETH-DAI pair not found!");
        
        address usdcDaiPair = IUniswapV2Factory(factory).getPair(usdc, dai);
        require(usdcDaiPair != address(0), "USDC-DAI pair not found!");

        console.log("");
        console.log("Adding liquidity to WETH-USDC pair...");
        console.log("  Pair address:", wethUsdcPair);
        console.log("  Adding WETH amount (wei):", wethForUsdc);
        console.log("  Adding USDC amount (wei):", usdcAmount);
        
        IERC20(weth).transfer(wethUsdcPair, wethForUsdc);
        IERC20(usdc).transfer(wethUsdcPair, usdcAmount);
        uint liquidity1 = IUniswapV2Pair(wethUsdcPair).mint(deployer);
        
        (uint112 r0, uint112 r1,) = IUniswapV2Pair(wethUsdcPair).getReserves();
        console.log("  Liquidity minted:", liquidity1);
        console.log("  Reserves after - r0:", r0);
        console.log("  Reserves after - r1:", r1);

        console.log("");
        console.log("Adding liquidity to WETH-DAI pair...");
        console.log("  Pair address:", wethDaiPair);
        console.log("  Adding WETH amount (wei):", wethForDai);
        console.log("  Adding DAI amount (wei):", daiAmount);
        
        IERC20(weth).transfer(wethDaiPair, wethForDai);
        IERC20(dai).transfer(wethDaiPair, daiAmount);
        uint liquidity2 = IUniswapV2Pair(wethDaiPair).mint(deployer);
        
        (r0, r1,) = IUniswapV2Pair(wethDaiPair).getReserves();
        console.log("  Liquidity minted:", liquidity2);
        console.log("  Reserves after - r0:", r0);
        console.log("  Reserves after - r1:", r1);

        console.log("");
        console.log("Adding liquidity to USDC-DAI pair...");
        console.log("  Pair address:", usdcDaiPair);
        console.log("  Adding USDC:", usdcForDai / 1e6);
        console.log("  Adding DAI:", daiForUsdc / 1e18);
        
        IERC20(usdc).transfer(usdcDaiPair, usdcForDai);
        IERC20(dai).transfer(usdcDaiPair, daiForUsdc);
        uint liquidity3 = IUniswapV2Pair(usdcDaiPair).mint(deployer);
        
        (r0, r1,) = IUniswapV2Pair(usdcDaiPair).getReserves();
        console.log("  Liquidity minted:", liquidity3);
        console.log("  Reserves after: r0 =", r0, "r1 =", r1);

        vm.stopBroadcast();

        console.log("");
        console.log("===========================================");
        console.log("LIQUIDITY ADDED SUCCESSFULLY");
        console.log("===========================================");
    }
}

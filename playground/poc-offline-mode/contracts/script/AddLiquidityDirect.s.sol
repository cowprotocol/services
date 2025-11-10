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
        
        // Load Uniswap factory
        address factory = vm.envAddress("UNISWAP_FACTORY");

        console.log("===========================================");
        console.log("ADDING LIQUIDITY TO UNISWAP V2 PAIRS");
        console.log("===========================================");
        console.log("Deployer:", deployer);
        console.log("Factory:", factory);
        console.log("");

        vm.startBroadcast(deployerPrivateKey);

        // Check initial balances
        console.log("Initial balances:");
        console.log("  ETH:", deployer.balance / 1e18, "ETH");
        console.log("  WETH:", IERC20(weth).balanceOf(deployer) / 1e18, "WETH");
        console.log("  USDC:", IERC20(usdc).balanceOf(deployer) / 1e6, "USDC");
        console.log("  DAI:", IERC20(dai).balanceOf(deployer) / 1e18, "DAI");
        console.log("");

        // Liquidity amounts (using realistic ratios)
        // WETH = $2000, USDC = $1, DAI = $1
        uint256 wethForUsdc = 10 ether;      // 10 WETH
        uint256 usdcAmount = 20_000 * 1e6;   // 20,000 USDC (6 decimals)
        
        uint256 wethForDai = 10 ether;       // 10 WETH
        uint256 daiAmount = 20_000 * 1e18;   // 20,000 DAI (18 decimals)
        
        uint256 usdcForDai = 10_000 * 1e6;   // 10,000 USDC
        uint256 daiForUsdc = 10_000 * 1e18;  // 10,000 DAI

        // Add WETH-USDC liquidity
        console.log("Adding WETH-USDC liquidity...");
        console.log("  WETH:", wethForUsdc / 1e18, "WETH");
        console.log("  USDC:", usdcAmount / 1e6, "USDC");
        address wethUsdcPair = IUniswapV2Factory(factory).getPair(weth, usdc);
        console.log("  Pair:", wethUsdcPair);
        
        IERC20(weth).transfer(wethUsdcPair, wethForUsdc);
        IERC20(usdc).transfer(wethUsdcPair, usdcAmount);
        uint liquidity1 = IUniswapV2Pair(wethUsdcPair).mint(deployer);
        console.log("  Liquidity minted:", liquidity1);
        
        (uint112 r0, uint112 r1,) = IUniswapV2Pair(wethUsdcPair).getReserves();
        console.log("  Reserves:", uint256(r0), uint256(r1));
        console.log("  WETH-USDC liquidity added");

        // Add WETH-DAI liquidity
        console.log("");
        console.log("Adding WETH-DAI liquidity...");
        console.log("  WETH:", wethForDai / 1e18, "WETH");
        console.log("  DAI:", daiAmount / 1e18, "DAI");
        address wethDaiPair = IUniswapV2Factory(factory).getPair(weth, dai);
        console.log("  Pair:", wethDaiPair);
        
        IERC20(weth).transfer(wethDaiPair, wethForDai);
        IERC20(dai).transfer(wethDaiPair, daiAmount);
        uint liquidity2 = IUniswapV2Pair(wethDaiPair).mint(deployer);
        console.log("  Liquidity minted:", liquidity2);
        
        (r0, r1,) = IUniswapV2Pair(wethDaiPair).getReserves();
        console.log("  Reserves:", uint256(r0), uint256(r1));
        console.log("  WETH-DAI liquidity added");

        // Add USDC-DAI liquidity
        console.log("");
        console.log("Adding USDC-DAI liquidity...");
        console.log("  USDC:", usdcForDai / 1e6, "USDC");
        console.log("  DAI:", daiForUsdc / 1e18, "DAI");
        address usdcDaiPair = IUniswapV2Factory(factory).getPair(usdc, dai);
        console.log("  Pair:", usdcDaiPair);
        
        IERC20(usdc).transfer(usdcDaiPair, usdcForDai);
        IERC20(dai).transfer(usdcDaiPair, daiForUsdc);
        uint liquidity3 = IUniswapV2Pair(usdcDaiPair).mint(deployer);
        console.log("  Liquidity minted:", liquidity3);
        
        (r0, r1,) = IUniswapV2Pair(usdcDaiPair).getReserves();
        console.log("  Reserves:", uint256(r0), uint256(r1));
        console.log("  USDC-DAI liquidity added");

        vm.stopBroadcast();

        console.log("");
        console.log("===========================================");
        console.log("LIQUIDITY ADDED SUCCESSFULLY");
        console.log("===========================================");
        console.log("All three pairs now have liquidity!");
        console.log("===========================================");
    }
}

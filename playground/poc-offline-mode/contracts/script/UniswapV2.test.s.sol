// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

// Uniswap V2 interfaces
interface IUniswapV2Factory {
    function getPair(address tokenA, address tokenB) external view returns (address pair);
}

interface IUniswapV2Pair {
    function token0() external view returns (address);
    function token1() external view returns (address);
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    function totalSupply() external view returns (uint);
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
    function swapExactTokensForTokens(
        uint amountIn,
        uint amountOutMin,
        address[] calldata path,
        address to,
        uint deadline
    ) external returns (uint[] memory amounts);
}

interface IWETH {
    function deposit() external payable;
    function approve(address spender, uint amount) external returns (bool);
    function balanceOf(address owner) external view returns (uint);
}

/// @title TestUniswapV2
/// @notice Test script to verify Uniswap V2 deployment works correctly
contract TestUniswapV2 is Script {
    function run() external {
        // Load deployer private key
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);

        // Load addresses
        address weth = vm.envAddress("WETH_ADDRESS");
        address usdc = vm.envAddress("USDC_ADDRESS");
        address dai = vm.envAddress("DAI_ADDRESS");
        address factory = vm.envAddress("UNISWAP_FACTORY");
        address router = vm.envAddress("UNISWAP_ROUTER");
        address pairWethUsdc = vm.envAddress("PAIR_WETH_USDC");

        console.log("===========================================");
        console.log("TESTING UNISWAP V2 DEPLOYMENT");
        console.log("===========================================");
        console.log("Tester:", deployer);
        console.log("");

        // Test 1: Verify Router configuration
        console.log("TEST 1: Router Configuration");
        console.log("-------------------------------------------");
        IUniswapV2Router02 routerContract = IUniswapV2Router02(router);
        address factoryFromRouter = routerContract.factory();
        address wethFromRouter = routerContract.WETH();
        
        console.log("  Router factory:", factoryFromRouter);
        console.log("  Expected factory:", factory);
        require(factoryFromRouter == factory, "Router factory mismatch");
        console.log("  OK Factory matches");
        
        console.log("  Router WETH:", wethFromRouter);
        console.log("  Expected WETH:", weth);
        require(wethFromRouter == weth, "Router WETH mismatch");
        console.log("  OK WETH matches");
        console.log("");

        // Test 2: Verify pair exists and has correct tokens
        console.log("TEST 2: Pair Verification");
        console.log("-------------------------------------------");
        IUniswapV2Factory factoryContract = IUniswapV2Factory(factory);
        address pairFromFactory = factoryContract.getPair(weth, usdc);
        
        console.log("  Pair from factory:", pairFromFactory);
        console.log("  Expected pair:", pairWethUsdc);
        require(pairFromFactory == pairWethUsdc, "Pair address mismatch");
        console.log("  OK Pair exists");
        
        IUniswapV2Pair pair = IUniswapV2Pair(pairWethUsdc);
        address token0 = pair.token0();
        address token1 = pair.token1();
        console.log("  Pair token0:", token0);
        console.log("  Pair token1:", token1);
        
        // Tokens should be sorted (token0 < token1)
        require(
            (token0 == weth && token1 == usdc) || (token0 == usdc && token1 == weth),
            "Pair has wrong tokens"
        );
        console.log("  OK Pair has correct tokens");
        console.log("");

        // Test 3: Add liquidity
        console.log("TEST 3: Add Liquidity");
        console.log("-------------------------------------------");
        
        uint256 wethAmount = 10 ether;
        uint256 usdcAmount = 30000 * 1e6; // 30,000 USDC (assuming ~$3000 per ETH)
        
        vm.startBroadcast(deployerPrivateKey);
        
        // Approve router to spend tokens
        console.log("  Approving WETH...");
        IWETH(weth).approve(router, wethAmount);
        console.log("  Approving USDC...");
        IERC20(usdc).approve(router, usdcAmount);
        
        // Check balances before
        uint256 wethBalanceBefore = IWETH(weth).balanceOf(deployer);
        uint256 usdcBalanceBefore = IERC20(usdc).balanceOf(deployer);
        console.log("  WETH balance before:", wethBalanceBefore / 1e18, "WETH");
        console.log("  USDC balance before:", usdcBalanceBefore / 1e6, "USDC");
        
        // Add liquidity
        console.log("  Adding liquidity...");
        (uint amountA, uint amountB, uint liquidity) = routerContract.addLiquidity(
            weth,
            usdc,
            wethAmount,
            usdcAmount,
            0, // amountAMin
            0, // amountBMin
            deployer,
            block.timestamp + 1 hours
        );
        
        console.log("  OK Liquidity added:");
        console.log("    WETH amount:", amountA / 1e18);
        console.log("    USDC amount:", amountB / 1e6);
        console.log("    LP tokens:", liquidity);
        
        // Check balances after
        uint256 wethBalanceAfter = IWETH(weth).balanceOf(deployer);
        uint256 usdcBalanceAfter = IERC20(usdc).balanceOf(deployer);
        console.log("  WETH balance after:", wethBalanceAfter / 1e18, "WETH");
        console.log("  USDC balance after:", usdcBalanceAfter / 1e6, "USDC");
        console.log("");

        // Test 4: Check reserves
        console.log("TEST 4: Verify Reserves");
        console.log("-------------------------------------------");
        (uint112 reserve0, uint112 reserve1,) = pair.getReserves();
        console.log("  Reserve0:", uint256(reserve0));
        console.log("  Reserve1:", uint256(reserve1));
        console.log("  Total LP supply:", pair.totalSupply());
        require(reserve0 > 0 && reserve1 > 0, "Reserves are empty");
        console.log("  OK Pair has liquidity");
        console.log("");

        // Test 5: Perform a small swap
        console.log("TEST 5: Test Swap");
        console.log("-------------------------------------------");
        
        uint256 swapAmount = 1 ether;
        console.log("  Swapping", swapAmount / 1e18, "WETH for USDC...");
        
        // Approve for swap
        IWETH(weth).approve(router, swapAmount);
        
        // Setup swap path
        address[] memory path = new address[](2);
        path[0] = weth;
        path[1] = usdc;
        
        uint256 usdcBefore = IERC20(usdc).balanceOf(deployer);
        
        // Perform swap
        uint[] memory amounts = routerContract.swapExactTokensForTokens(
            swapAmount,
            0, // amountOutMin
            path,
            deployer,
            block.timestamp + 1 hours
        );
        
        uint256 usdcAfter = IERC20(usdc).balanceOf(deployer);
        uint256 usdcReceived = usdcAfter - usdcBefore;
        
        console.log("  OK Swap successful:");
        console.log("    WETH in:", amounts[0] / 1e18);
        console.log("    USDC out:", amounts[1] / 1e6);
        console.log("    USDC received:", usdcReceived / 1e6);
        console.log("");

        vm.stopBroadcast();

        console.log("===========================================");
        console.log("OK ALL TESTS PASSED");
        console.log("===========================================");
        console.log("Uniswap V2 is working correctly!");
        console.log("  - Router configured properly");
        console.log("  - Pairs created correctly");
        console.log("  - Can add liquidity");
        console.log("  - Can perform swaps");
        console.log("===========================================");
    }
}

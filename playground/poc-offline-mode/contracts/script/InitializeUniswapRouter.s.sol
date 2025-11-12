// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

interface IERC20 {
    function approve(address spender, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

interface IUniswapV2Router02 {
    function WETH() external pure returns (address);
    function factory() external pure returns (address);
}

/// @title InitializeUniswapRouter
/// @notice Approve the router to spend tokens by directly setting ERC20 allowance storage
contract InitializeUniswapRouter is Script {
    function run() external {
        // Load deployer private key
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);

        // Load addresses
        address weth = vm.envAddress("WETH_ADDRESS");
        address usdc = vm.envAddress("USDC_ADDRESS");
        address dai = vm.envAddress("DAI_ADDRESS");
        address router = vm.envAddress("UNISWAP_ROUTER");
        address settlement = vm.envAddress("COW_SETTLEMENT");

        console.log("===========================================");
        console.log("INITIALIZING UNISWAP ROUTER");
        console.log("===========================================");
        console.log("Deployer:", deployer);
        console.log("Settlement:", settlement);
        console.log("Router:", router);
        console.log("");
        console.log("Tokens to approve:");
        console.log("  WETH:", weth);
        console.log("  USDC:", usdc);
        console.log("  DAI:", dai);
        console.log("");

        // Verify router can access WETH and factory
        try IUniswapV2Router02(router).WETH() returns (address wethFromRouter) {
            console.log("Router WETH check passed:", wethFromRouter);
            require(wethFromRouter == weth, "Router WETH mismatch!");
        } catch {
            console.log("ERROR: Router WETH() call failed!");
            revert("Router initialization failed");
        }

        try IUniswapV2Router02(router).factory() returns (address factoryFromRouter) {
            console.log("Router factory check passed:", factoryFromRouter);
        } catch {
            console.log("ERROR: Router factory() call failed!");
            revert("Router factory check failed");
        }

        console.log("");
        console.log("Approving ROUTER to spend tokens from SETTLEMENT contract...");
        console.log("Router address:", router);
        
        uint256 maxAmount = type(uint256).max;
        bytes32 maxAmountBytes = bytes32(maxAmount);
        
        // ERC20 storage layout: mapping(address => mapping(address => uint256)) public allowance
        // Storage slot for allowance[settlement][router] = keccak256(abi.encode(router, keccak256(abi.encode(settlement, 0))))
        // Slot 0 = balanceOf mapping, Slot 1 = allowance mapping (for standard ERC20)
        
        // For WETH: allowance slot
        bytes32 wethSlot = keccak256(abi.encode(router, keccak256(abi.encode(settlement, uint256(1)))));
        vm.store(weth, wethSlot, maxAmountBytes);
        console.log("  WETH approved to router from settlement");

        // For USDC: allowance slot
        bytes32 usdcSlot = keccak256(abi.encode(router, keccak256(abi.encode(settlement, uint256(1)))));
        vm.store(usdc, usdcSlot, maxAmountBytes);
        console.log("  USDC approved to router from settlement");

        // For DAI: allowance slot
        bytes32 daiSlot = keccak256(abi.encode(router, keccak256(abi.encode(settlement, uint256(1)))));
        vm.store(dai, daiSlot, maxAmountBytes);
        console.log("  DAI approved to router from settlement");

        console.log("");
        console.log("===========================================");
        console.log("UNISWAP ROUTER INITIALIZED");
        console.log("===========================================");
    }
}

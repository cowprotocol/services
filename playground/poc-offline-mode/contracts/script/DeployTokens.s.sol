// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";
import {WETH} from "solmate/tokens/WETH.sol";
import {TestERC20} from "../src/tokens/TestERC20.sol";

/// @title DeployTokens
/// @notice Deploy WETH, USDC, and DAI for PoC
contract DeployTokens is Script {
    // Token supply constants
    // Note: Anvil accounts start with 10,000 ETH, but some is spent on gas
    uint256 constant WETH_SUPPLY = 1_000 ether; // 1,000 WETH
    uint256 constant USDC_SUPPLY = 1_000_000 * 1e6; // 1 million USDC
    uint256 constant DAI_SUPPLY = 1_000_000 ether; // 1 million DAI

    function run() external {
        // Load deployer private key
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);

        console.log("===========================================");
        console.log("DEPLOYING TOKENS");
        console.log("===========================================");
        console.log("Deployer:", deployer);
        console.log("Chain ID:", block.chainid);
        console.log("");

        vm.startBroadcast(deployerPrivateKey);

        // Deploy WETH
        console.log("Deploying WETH...");
        WETH weth = new WETH();
        console.log("  WETH deployed at:", address(weth));
        
        // Wrap some ETH to WETH for deployer
        weth.deposit{value: WETH_SUPPLY}();
        console.log("  Wrapped", WETH_SUPPLY / 1e18, "ETH to WETH");

        // Deploy USDC (6 decimals)
        console.log("");
        console.log("Deploying USDC...");
        TestERC20 usdc = new TestERC20(
            "USD Coin",
            "USDC",
            6,
            USDC_SUPPLY
        );
        console.log("  USDC deployed at:", address(usdc));
        console.log("  Initial supply:", USDC_SUPPLY / 1e6, "USDC");

        // Deploy DAI (18 decimals)
        console.log("");
        console.log("Deploying DAI...");
        TestERC20 dai = new TestERC20(
            "Dai Stablecoin",
            "DAI",
            18,
            DAI_SUPPLY
        );
        console.log("  DAI deployed at:", address(dai));
        console.log("  Initial supply:", DAI_SUPPLY / 1e18, "DAI");

        vm.stopBroadcast();

        console.log("");
        console.log("===========================================");
        console.log("DEPLOYMENT SUMMARY");
        console.log("===========================================");
        console.log("WETH:", address(weth));
        console.log("USDC:", address(usdc));
        console.log("DAI:", address(dai));
        console.log("");
        console.log("All tokens deployed to:", deployer);
        console.log("===========================================");
    }
}

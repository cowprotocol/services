// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";
import {WETH} from "solmate/tokens/WETH.sol";
import {TestERC20} from "../src/tokens/TestERC20.sol";
import {DeploymentUtils} from "./Utils.sol";

/// @title DeployTokens
/// @notice Deploy WETH, USDC, DAI, USDT, and GNO for PoC using CREATE2
contract DeployTokens is Script {
    // Token supply constants
    // Note: Anvil accounts start with 10,000 ETH, but some is spent on gas
    uint256 constant WETH_SUPPLY = 1_000 ether; // 1,000 WETH
    uint256 constant USDC_SUPPLY = 1_000_000 * 1e6; // 1 million USDC
    uint256 constant DAI_SUPPLY = 1_000_000 ether; // 1 million DAI
    uint256 constant USDT_SUPPLY = 1_000_000 * 1e6; // 1 million USDT (6 decimals)
    uint256 constant GNO_SUPPLY = 1_000_000 ether; // 1 million GNO (18 decimals)

    // Deterministic salts for CREATE2
    bytes32 constant WETH_SALT = keccak256("token-weth");
    bytes32 constant USDC_SALT = keccak256("token-usdc");
    bytes32 constant DAI_SALT = keccak256("token-dai");
    bytes32 constant USDT_SALT = keccak256("token-usdt");
    bytes32 constant GNO_SALT = keccak256("token-gno");

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

        // Deploy WETH with CREATE2
        console.log("Deploying WETH with CREATE2...");
        WETH weth = new WETH{salt: WETH_SALT}();
        console.log("  WETH deployed at:", address(weth));

        // Wrap some ETH to WETH for deployer
        weth.deposit{value: WETH_SUPPLY}();
        console.log("  Wrapped", WETH_SUPPLY / 1e18, "ETH to WETH");

        // Deploy USDC (6 decimals) with CREATE2
        console.log("");
        console.log("Deploying USDC with CREATE2...");
        TestERC20 usdc = new TestERC20{salt: USDC_SALT}(
            "USD Coin",
            "USDC",
            6,
            0 // Initial supply 0, will mint to deployer next
        );
        usdc.mint(deployer, USDC_SUPPLY);
        console.log("  USDC deployed at:", address(usdc));
        console.log("  Initial supply:", USDC_SUPPLY / 1e6, "USDC");

        // Deploy DAI (18 decimals) with CREATE2
        console.log("");
        console.log("Deploying DAI with CREATE2...");
        TestERC20 dai = new TestERC20{salt: DAI_SALT}(
            "Dai Stablecoin",
            "DAI",
            18,
            0 // Initial supply 0, will mint to deployer next
        );
        dai.mint(deployer, DAI_SUPPLY);
        console.log("  DAI deployed at:", address(dai));
        console.log("  Initial supply:", DAI_SUPPLY / 1e18, "DAI");

        // Deploy USDT (6 decimals) with CREATE2
        console.log("");
        console.log("Deploying USDT with CREATE2...");
        TestERC20 usdt = new TestERC20{salt: USDT_SALT}(
            "Tether USD",
            "USDT",
            6,
            0 // Initial supply 0, will mint to deployer next
        );
        usdt.mint(deployer, USDT_SUPPLY);
        console.log("  USDT deployed at:", address(usdt));
        console.log("  Initial supply:", USDT_SUPPLY / 1e6, "USDT");

        // Deploy GNO (18 decimals) with CREATE2
        console.log("");
        console.log("Deploying GNO with CREATE2...");
        TestERC20 gno = new TestERC20{salt: GNO_SALT}(
            "Gnosis Token",
            "GNO",
            18,
            0 // Initial supply 0, will mint to deployer next
        );
        gno.mint(deployer, GNO_SUPPLY);
        console.log("  GNO deployed at:", address(gno));
        console.log("  Initial supply:", GNO_SUPPLY / 1e18, "GNO");

        vm.stopBroadcast();

        console.log("");
        console.log("===========================================");
        console.log("DEPLOYMENT SUMMARY");
        console.log("===========================================");
        console.log("WETH:", address(weth));
        console.log("USDC:", address(usdc));
        console.log("DAI:", address(dai));
        console.log("USDT:", address(usdt));
        console.log("GNO:", address(gno));
        console.log("");
        console.log("All tokens deployed to:", deployer);
        console.log("===========================================");
    }
}

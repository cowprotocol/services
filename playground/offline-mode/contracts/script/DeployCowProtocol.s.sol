
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Script.sol";
import {DeploymentUtils} from "./Utils.sol";

contract DeployCowProtocol is Script {
    using DeploymentUtils for bytes;

    // Deterministic salts for CREATE2
    bytes32 constant AUTHENTICATOR_SALT = keccak256("cowswap-authenticator");
    bytes32 constant SETTLEMENT_SALT = keccak256("cowswap-settlement");

    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);
        address balancerVault = vm.envAddress("BALANCER_VAULT_ADDRESS");

        console.log("Deploying CoW Protocol contracts with CREATE2...");
        console.log("Deployer:", deployer);
        console.log("Balancer Vault:", balancerVault);

        vm.startBroadcast(deployerPrivateKey);

        // Deploy GPv2AllowListAuthentication Implementation using CREATE2
        // Using official precompiled Implementation contract (not proxy) from npm package
        console.log("Deploying GPv2AllowListAuthentication Implementation with CREATE2...");
        string memory authenticatorJson = vm.readFile("node_modules/@cowprotocol/contracts/deployments/xdai/GPv2AllowListAuthentication_Implementation.json");
        bytes memory authenticatorBytecode = vm.parseJsonBytes(authenticatorJson, ".bytecode");
        address authenticator = DeploymentUtils.deployWithCreate2(authenticatorBytecode, AUTHENTICATOR_SALT);
        console.log("GPv2AllowListAuthentication deployed at:", authenticator);

        // Deploy GPv2Settlement using CREATE2
        // Using official precompiled bytecode from npm package
        console.log("Deploying GPv2Settlement with CREATE2...");
        string memory settlementJson = vm.readFile("node_modules/@cowprotocol/contracts/deployments/xdai/GPv2Settlement.json");
        bytes memory settlementCreationCode = vm.parseJsonBytes(settlementJson, ".bytecode");
        bytes memory settlementBytecode = abi.encodePacked(
            settlementCreationCode,
            abi.encode(authenticator, balancerVault)
        );
        address settlement = DeploymentUtils.deployWithCreate2(settlementBytecode, SETTLEMENT_SALT);
        console.log("GPv2Settlement deployed at:", settlement);

        vm.stopBroadcast();
        
        // Get the VaultRelayer address from Settlement
        // The Settlement contract creates it automatically
        bytes memory vaultRelayerCalldata = abi.encodeWithSignature("vaultRelayer()");
        (bool success, bytes memory vaultRelayerData) = settlement.staticcall(vaultRelayerCalldata);
        require(success, "Failed to get vaultRelayer address");
        address vaultRelayer = abi.decode(vaultRelayerData, (address));
        console.log("GPv2VaultRelayer (created by Settlement):", vaultRelayer);
        
        console.log("\nCoW Protocol deployment complete!");
    }
}

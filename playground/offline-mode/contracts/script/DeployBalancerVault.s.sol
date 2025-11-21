// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Script.sol";
import {MockBalancerVault} from "../src/MockBalancerVault.sol";

/// @title DeployBalancerVault
/// @notice Deploy MockBalancerVault with CREATE2 for deterministic address
contract DeployBalancerVault is Script {
    // Deterministic salt for CREATE2
    bytes32 constant BALANCER_VAULT_SALT = keccak256("mock-balancer-vault");

    function run() public {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");

        console.log("Deploying MockBalancerVault with CREATE2...");

        vm.startBroadcast(deployerPrivateKey);

        // Deploy MockBalancerVault contract with CREATE2 for deterministic address
        MockBalancerVault vault = new MockBalancerVault{salt: BALANCER_VAULT_SALT}();

        console.log("MockBalancerVault deployed at:", address(vault));

        vm.stopBroadcast();
    }
}

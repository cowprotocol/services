// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Script.sol";

contract DeployCowProtocol is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);
        address balancerVault = vm.envAddress("BALANCER_VAULT_ADDRESS");
        
        console.log("Deploying CoW Protocol contracts...");
        console.log("Deployer:", deployer);
        console.log("Balancer Vault:", balancerVault);
        
        vm.startBroadcast(deployerPrivateKey);
        
        // Deploy GPv2AllowListAuthentication
        // Constructor: no parameters
        address authenticator = deployCode(
            "contracts/out-cow-protocol/GPv2AllowListAuthentication.sol/GPv2AllowListAuthentication.json",
            ""
        );
        console.log("GPv2AllowListAuthentication deployed at:", authenticator);
        
        // Deploy GPv2Settlement
        // Constructor: GPv2Authentication _authenticator, IVault _vault
        // NOTE: The Settlement contract creates its own VaultRelayer in the constructor
        address settlement = deployCode(
            "contracts/out-cow-protocol/GPv2Settlement.sol/GPv2Settlement.json",
            abi.encode(authenticator, balancerVault)
        );
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

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Script.sol";

// Interface for GPv2AllowListAuthentication
interface IGPv2Authentication {
    function manager() external view returns (address);
    function isSolver(address prospectiveSolver) external view returns (bool);
    function initializeManager(address manager_) external;
    function addSolver(address solver) external;
}

// Interface for GPv2Settlement
interface IGPv2Settlement {
    function authenticator() external view returns (address);
    function vault() external view returns (address);
    function vaultRelayer() external view returns (address);
    function domainSeparator() external view returns (bytes32);
}

// Interface for GPv2VaultRelayer
interface IGPv2VaultRelayer {
    function vault() external view returns (address);
}

contract TestCowProtocol is Script {
    function run() external view {
        // Load addresses from environment
        address authenticator = vm.envAddress("COW_AUTHENTICATOR");
        address vaultRelayer = vm.envAddress("COW_VAULT_RELAYER");
        address settlement = vm.envAddress("COW_SETTLEMENT");
        address balancerVault = vm.envAddress("BALANCER_VAULT");
        address deployer = vm.envAddress("DEPLOYER_ADDRESS");
        
        console.log("========================================");
        console.log("Testing CoW Protocol Deployment");
        console.log("========================================");
        console.log("");
        
        // Test 1: Check contract addresses are not zero
        console.log("Test 1: Verify contract addresses");
        require(authenticator != address(0), "Authenticator address is zero");
        require(vaultRelayer != address(0), "VaultRelayer address is zero");
        require(settlement != address(0), "Settlement address is zero");
        console.log("  Authenticator:", authenticator);
        console.log("  VaultRelayer:", vaultRelayer);
        console.log("  Settlement:", settlement);
        console.log("  ✅ All addresses are valid");
        console.log("");
        
        // Test 2: Check contracts have code
        console.log("Test 2: Verify contracts are deployed");
        require(authenticator.code.length > 0, "Authenticator has no code");
        require(vaultRelayer.code.length > 0, "VaultRelayer has no code");
        require(settlement.code.length > 0, "Settlement has no code");
        console.log("  Authenticator code size:", authenticator.code.length, "bytes");
        console.log("  VaultRelayer code size:", vaultRelayer.code.length, "bytes");
        console.log("  Settlement code size:", settlement.code.length, "bytes");
        console.log("  ✅ All contracts have bytecode");
        console.log("");
        
        // Test 3: Verify Settlement configuration
        console.log("Test 3: Verify Settlement configuration");
        IGPv2Settlement settlementContract = IGPv2Settlement(settlement);
        
        address settlementAuthenticator = settlementContract.authenticator();
        address settlementVault = settlementContract.vault();
        address settlementVaultRelayer = settlementContract.vaultRelayer();
        
        console.log("  Settlement.authenticator():", settlementAuthenticator);
        console.log("  Settlement.vault():", settlementVault);
        console.log("  Settlement.vaultRelayer():", settlementVaultRelayer);
        
        require(settlementAuthenticator == authenticator, "Settlement authenticator mismatch");
        require(settlementVault == balancerVault, "Settlement vault mismatch");
        require(settlementVaultRelayer == vaultRelayer, "Settlement vaultRelayer mismatch");
        console.log("  ✅ Settlement is correctly configured");
        console.log("");
        
        // Test 4: Verify VaultRelayer configuration
        console.log("Test 4: Verify VaultRelayer configuration");
        IGPv2VaultRelayer vaultRelayerContract = IGPv2VaultRelayer(vaultRelayer);
        
        address vaultRelayerVault = vaultRelayerContract.vault();
        console.log("  VaultRelayer.vault():", vaultRelayerVault);
        
        require(vaultRelayerVault == balancerVault, "VaultRelayer vault mismatch");
        console.log("  ✅ VaultRelayer is correctly configured");
        console.log("");
        
        // Test 5: Check domain separator (important for order signing)
        console.log("Test 5: Verify domain separator");
        bytes32 domainSeparator = settlementContract.domainSeparator();
        console.log("  Domain separator:", vm.toString(domainSeparator));
        require(domainSeparator != bytes32(0), "Domain separator is zero");
        console.log("  ✅ Domain separator is initialized");
        console.log("");
        
        // Test 6: Check Authenticator manager
        console.log("Test 6: Verify Authenticator manager");
        IGPv2Authentication authContract = IGPv2Authentication(authenticator);
        
        address manager = authContract.manager();
        console.log("  Current manager:", manager);
        
        if (manager == address(0)) {
            console.log("  ⚠️  Manager is not initialized");
            console.log("  To initialize, run:");
            console.log("    cast send", authenticator);
            console.log("      'initializeManager(address)'", deployer);
            console.log("      --rpc-url http://localhost:8545");
            console.log("      --private-key <DEPLOYER_KEY>");
        } else {
            console.log("  ✅ Manager is initialized");
            
            // Test 7: Check if deployer is a solver
            console.log("");
            console.log("Test 7: Check solver status");
            bool isDeployerSolver = authContract.isSolver(deployer);
            console.log("  Is deployer a solver?", isDeployerSolver);
            
            if (!isDeployerSolver) {
                console.log("  ℹ️  Deployer is not a solver yet");
                console.log("  To add deployer as solver, run:");
                console.log("    cast send", authenticator);
                console.log("      'addSolver(address)'", deployer);
                console.log("      --rpc-url http://localhost:8545");
                console.log("      --private-key <MANAGER_KEY>");
            } else {
                console.log("  ✅ Deployer is a solver");
            }
        }
        
        console.log("");
        console.log("========================================");
        console.log("✅ CoW Protocol Deployment Tests PASSED");
        console.log("========================================");
        console.log("");
        console.log("Summary:");
        console.log("  - All contracts deployed successfully");
        console.log("  - Settlement correctly references authenticator and vault");
        console.log("  - VaultRelayer correctly references vault");
        console.log("  - Domain separator initialized for order signing");
        console.log("");
        console.log("Next steps:");
        console.log("  1. Initialize the manager (if not done)");
        console.log("  2. Add solvers to the allow list");
        console.log("  3. Test order settlement");
    }
}

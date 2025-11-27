// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "forge-std/Script.sol";

// Import GPv2TradeSimulator (Solidity 0.7.6)
// We can't use a direct import here due to Solidity version mismatch
// Instead, we'll use inline assembly/CREATE2 or deploy via bytecode

contract DeployTradeSimulator is Script {
    // Deterministic salt for CREATE2
    bytes32 constant TRADE_SIMULATOR_SALT = keccak256("cowswap-trade-simulator");

    function run() public {
        vm.startBroadcast();

        // Deploy GPv2TradeSimulator using CREATE2
        // Since we can't directly import the 0.7.6 contract in a 0.8.17 script,
        // we'll deploy it using the artifact bytecode

        // Get the creation bytecode from the compiled artifact
        // GPv2TradeSimulator is compiled with the cow-protocol profile, so it's in out-cow-protocol
        bytes memory bytecode = vm.getCode("contracts/out-cow-protocol/GPv2TradeSimulator.sol/GPv2TradeSimulator.json");

        bytes32 salt = TRADE_SIMULATOR_SALT;
        address tradeSimulator;
        assembly {
            tradeSimulator := create2(
                0,
                add(bytecode, 0x20),
                mload(bytecode),
                salt
            )
        }

        require(tradeSimulator != address(0), "Deployment failed");

        console.log("GPv2TradeSimulator deployed at:", tradeSimulator);

        vm.stopBroadcast();
    }
}

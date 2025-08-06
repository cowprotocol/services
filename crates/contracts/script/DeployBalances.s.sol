// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "solidity/Balances.sol";
import "forge-std/Script.sol";

contract DeployBalancesScript is Script {
    function run() public {
        vm.broadcast();
        new Balances();
    }
}

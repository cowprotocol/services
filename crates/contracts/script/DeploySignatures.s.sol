// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "solidity/Signatures.sol";
import "forge-std/Script.sol";

contract DeploySignaturesScript is Script {
    function run() public {
        vm.broadcast();
        new Signatures();
    }
}

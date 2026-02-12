// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @title Minimal forwarder for EIP-7702 delegation
/// @notice Forwards any call to a fixed target address using CALL.
/// In an EIP-7702 context the delegating EOA executes this code, so
/// msg.sender seen by the target is the EOA itself (not the original caller).
contract Forwarder {
    address public immutable target;

    constructor(address _target) {
        target = _target;
    }

    fallback() external payable {
        address t = target;
        assembly {
            calldatacopy(0, 0, calldatasize())
            let result := call(gas(), t, callvalue(), 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())
            switch result
            case 0 { revert(0, returndatasize()) }
            default { return(0, returndatasize()) }
        }
    }

    receive() external payable {}
}

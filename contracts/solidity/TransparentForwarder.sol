// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.30;

/// @title EIP-7702 Forwarder, designed for use with settlements by multiple addresses simultaneously on-chain.
/// @author CoW Protocol Developers
/// @notice When used with EIP-7702, the solver EOA delegates its code to this
/// contract. Approved submission EOAs call this contract with `abi.encode(target, data)` to execute
/// settlement transactions through the solver EOA, preserving `msg.sender = solver EOA` from the target's perspective.
contract TransparentForwarder {
    error Unauthorized(address sender);

    address private immutable APPROVED_CALLERS_0;
    address private immutable APPROVED_CALLERS_1;
    address private immutable APPROVED_CALLERS_2;
    address private immutable APPROVED_CALLERS_3;
    address private immutable APPROVED_CALLERS_4;

    constructor(address[5] memory approvedCallers) {
        APPROVED_CALLERS_0 = approvedCallers[0];
        APPROVED_CALLERS_1 = approvedCallers[1];
        APPROVED_CALLERS_2 = approvedCallers[2];
        APPROVED_CALLERS_3 = approvedCallers[3];
        APPROVED_CALLERS_4 = approvedCallers[4];
    }

    /**
     * @dev Sends a call to 
     *
     * This function does not return to its internal call site, it will return directly to the external caller.
     */
    function _callThrough() internal virtual {
        // For our purposes, the target address is encoded as the first 20 bytes of the input data
        address target = address(bytes20(msg.data[0:20]));
        assembly {
            // Copy msg.data. We take full control of memory in this inline assembly
            // block because it will not return to Solidity code. We overwrite the
            // Solidity scratch pad at memory position 0.
            calldatacopy(0x00, 20, sub(calldatasize(), 20))

            // Call the implementation.
            // out and outsize are 0 because we don't know the size yet.
            let result := call(gas(), target, callvalue(), 0x00, sub(calldatasize(), 20), 0x00, 0x00)

            // Copy the returned data.
            returndatacopy(0x00, 0x00, returndatasize())

            switch result
            // call returns 0 on error.
            case 0 {
                revert(0x00, returndatasize())
            }
            default {
                return(0x00, returndatasize())
            }
        }
    }

    fallback() external payable {
        if(msg.data.length < 20) {
            // do nothing and receives ETH if that is what is happening
            return;
        }

        if (msg.sender == APPROVED_CALLERS_0) {
            return _callThrough();
        }

        if (msg.sender == APPROVED_CALLERS_1) {
            return _callThrough();
        }

        if (msg.sender == APPROVED_CALLERS_2) {
            return _callThrough();
        }

        if (msg.sender == APPROVED_CALLERS_3) {
            return _callThrough();
        }

        if (msg.sender == APPROVED_CALLERS_4) {
            return _callThrough();
        }

        revert Unauthorized(msg.sender);
    }
}

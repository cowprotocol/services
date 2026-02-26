// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.28;

/// @title EIP-7702 Settlement Forwarder for CoW Protocol
/// @notice When used with EIP-7702, the solver EOA delegates its code to this
/// contract. Approved submission EOAs call `forward(target, data)` to execute
/// settlement transactions through the solver EOA, preserving
/// `msg.sender = solver EOA` from the target's perspective.
///
/// Storage lives in the solver EOA's account (EIP-7702 semantics). The contract
/// is deployed once and shared across all solver EOAs — each gets its own
/// independent `isApprovedCaller` mapping in its own storage.
contract CowSettlementForwarder {
    mapping(address => bool) public isApprovedCaller;

    event ApprovedCallerSet(address indexed caller, bool approved);

    error Unauthorized();

    /// @notice Forward `data` to `target` via CALL.
    /// @dev Only approved callers can invoke this. In EIP-7702 context,
    /// `address(this)` = solver EOA, so `target` sees `msg.sender = solver EOA`.
    function forward(address target, bytes calldata data) external payable {
        if (!isApprovedCaller[msg.sender]) revert Unauthorized();
        (bool success, bytes memory result) = target.call{value: msg.value}(data);
        assembly {
            switch success
            case 0 { revert(add(result, 32), mload(result)) }
            default { return(add(result, 32), mload(result)) }
        }
    }

    /// @notice Set approved callers.
    function setApprovedCallers(address[] calldata callers, bool approved) external {
        if (msg.sender != address(this)) revert Unauthorized();
        for (uint256 i = 0; i < callers.length; i++) {
            isApprovedCaller[callers[i]] = approved;
            emit ApprovedCallerSet(callers[i], approved);
        }
    }
}

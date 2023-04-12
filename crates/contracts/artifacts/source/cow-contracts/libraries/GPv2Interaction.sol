// SPDX-License-Identifier: LGPL-3.0-or-later
pragma solidity ^0.7.6;

/// @title Gnosis Protocol v2 Interaction Library
/// @author Gnosis Developers
library GPv2Interaction {
    /// @dev Interaction data for performing arbitrary contract interactions.
    /// Submitted to [`GPv2Settlement.settle`] for code execution.
    struct Data {
        address target;
        uint256 value;
        bytes callData;
    }

    /// @dev Execute an arbitrary contract interaction.
    ///
    /// @param interaction Interaction data.
    function execute(Data calldata interaction) internal {
        address target = interaction.target;
        uint256 value = interaction.value;
        bytes calldata callData = interaction.callData;

        // NOTE: Use assembly to call the interaction instead of a low level
        // call for two reasons:
        // - We don't want to copy the return data, since we discard it for
        // interactions.
        // - Solidity will under certain conditions generate code to copy input
        // calldata twice to memory (the second being a "memcopy loop").
        // <https://github.com/gnosis/gp-v2-contracts/pull/417#issuecomment-775091258>
        // solhint-disable-next-line no-inline-assembly
        assembly {
            let freeMemoryPointer := mload(0x40)
            calldatacopy(freeMemoryPointer, callData.offset, callData.length)
            if iszero(
                call(
                    gas(),
                    target,
                    value,
                    freeMemoryPointer,
                    callData.length,
                    0,
                    0
                )
            ) {
                returndatacopy(0, 0, returndatasize())
                revert(0, returndatasize())
            }
        }
    }

    /// @dev Extracts the Solidity ABI selector for the specified interaction.
    ///
    /// @param interaction Interaction data.
    /// @return result The 4 byte function selector of the call encoded in
    /// this interaction.
    function selector(Data calldata interaction)
        internal
        pure
        returns (bytes4 result)
    {
        bytes calldata callData = interaction.callData;
        if (callData.length >= 4) {
            // NOTE: Read the first word of the interaction's calldata. The
            // value does not need to be shifted since `bytesN` values are left
            // aligned, and the value does not need to be masked since masking
            // occurs when the value is accessed and not stored:
            // <https://docs.soliditylang.org/en/v0.7.6/abi-spec.html#encoding-of-indexed-event-parameters>
            // <https://docs.soliditylang.org/en/v0.7.6/assembly.html#access-to-external-variables-functions-and-libraries>
            // solhint-disable-next-line no-inline-assembly
            assembly {
                result := calldataload(callData.offset)
            }
        }
    }
}

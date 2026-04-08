// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { ERC1271_MAGICVALUE, IERC1271 } from "./interfaces/IERC1271.sol";
import { ISettlement, Interaction } from "./interfaces/ISettlement.sol";
import { IVaultRelayer } from "./interfaces/IVaultRelayer.sol";

/// @title A contract for simulating available balances for settlements.
contract Signatures {
    /// @dev The on-chain contracts required for the simulation.
    struct Contracts {
        ISettlement settlement;
        IVaultRelayer vaultRelayer;
    }

    /// @dev Validates an ERC-1271 signature.
    ///
    /// @param contracts - On-chain contract addresses required for the
    /// simulation.
    /// @param signer - The address of the signer.
    /// @param order - The digest of the order to verify the signature for.
    /// @param signature - The ERC-1271 signature bytes.
    /// @param interactions - A list of pre-interactions required for setting
    /// up signature requirements.
    ///
    /// @return gasUsed - The gas units used for verifying the signature.
    function validate(
        Contracts memory contracts,
        IERC1271 signer,
        bytes32 order,
        bytes calldata signature,
        Interaction[] calldata interactions
    ) external returns (
        uint256 gasUsed
    ) {
        // Execute the interactions within the current context. This ensures
        // that any pre-interactions required for the signature (such as adding
        // a Composable CoW order) are executed before signature validation.
        executeInteractions(contracts, interactions);

        gasUsed = gasleft();
        bytes4 magicValue = signer.isValidSignature(order, signature);
        gasUsed = gasUsed - gasleft();

        require(magicValue == ERC1271_MAGICVALUE, "didn't say the magic word");
    }

    /// @dev Execute a set of interactions. This code is ported from the CoW
    /// Protocol settlement contract with minor modifications:
    /// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/GPv2Settlement.sol#L448-L470>
    /// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/libraries/GPv2Interaction.sol#L15-L49>
    function executeInteractions(
        Contracts memory contracts,
        Interaction[] calldata interactions
    ) private {
        require(
            address(this) == address(contracts.settlement),
            "incorrect calling context"
        );

        for (uint256 i; i < interactions.length; i++) {
            address target = interactions[i].target;
            uint256 value = interactions[i].value;
            bytes calldata callData = interactions[i].callData;

            require(
                target != address(contracts.vaultRelayer),
                "GPv2: forbidden interaction"
            );

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
    }
}

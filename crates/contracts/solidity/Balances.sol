// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IERC20 } from "./interfaces/IERC20.sol";
import { ISettlement } from "./interfaces/ISettlement.sol";
import { IVault } from "./interfaces/IVault.sol";
import { Transfer, IVaultRelayer } from "./interfaces/IVaultRelayer.sol";

/// @title A contract for simulating available balances for settlements.
contract Balances {
    ISettlement public immutable settlement;
    IVaultRelayer public immutable vaultRelayer;
    IVault public immutable vault;

    constructor(
        ISettlement _settlement,
        IVaultRelayer _vaultRelayer,
        IVault _vault
    ) {
        settlement = _settlement;
        vaultRelayer = _vaultRelayer;
        vault = _vault;
    }

    struct Interaction {
        address target;
        bytes callData;
    }

    function balanceErc20(
        address trader,
        IERC20 token,
        Interaction[] calldata interactions
    ) external returns (
        uint256 tokenBalance,
        uint256 allowance
    ) {
        executeInteractions(interactions);
        tokenBalance = token.balanceOf(trader);
        allowance = token.allowance(trader, address(vaultRelayer));
    }

    function balanceExternal(
        address trader,
        IERC20 token,
        Interaction[] calldata interactions
    ) external returns (
        uint256 tokenBalance,
        uint256 allowance
    ) {
        executeInteractions(interactions);
        tokenBalance = token.balanceOf(trader);
        allowance = vault
            .hasApprovedRelayer(trader, address(vaultRelayer))
                ? token.allowance(trader, address(vault))
                : 0;
    }

    function balanceInternal(
        address trader,
        IERC20 token,
        Interaction[] calldata interactions
    ) external returns (
        uint256 tokenBalance,
        uint256 allowance
    ) {
        executeInteractions(interactions);
        address[] memory tokens = new address[](1);
        tokens[0] = address(token);
        tokenBalance = vault.getInternalBalance(trader, tokens)[0];
        allowance = vault
            .hasApprovedRelayer(trader, address(vaultRelayer))
                ? type(uint256).max
                : 0;
    }

    /// @dev Execute a set of interactions. This code is ported from the CoW
    /// Protocol settlement contract with minor modifications:
    /// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/GPv2Settlement.sol#L448-L470>
    /// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/libraries/GPv2Interaction.sol#L15-L49>
    function executeInteractions(
        Interaction[] calldata interactions
    ) private {
        for (uint256 i; i < interactions.length; i++) {
            address target = interactions[i].target;
            uint256 value = 0;
            bytes calldata callData = interactions[i].callData;

            require(
                target != address(vaultRelayer),
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

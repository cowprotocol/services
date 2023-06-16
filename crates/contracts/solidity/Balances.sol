// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IERC20 } from "./interfaces/IERC20.sol";
import { ISettlement, Interaction } from "./interfaces/ISettlement.sol";
import { IVault } from "./interfaces/IVault.sol";
import { Transfer, IVaultRelayer } from "./interfaces/IVaultRelayer.sol";

/// @title A contract for simulating available balances for settlements.
contract Balances {
    /// The on-chain 
    struct Contracts {
        ISettlement settlement;
        IVaultRelayer vaultRelayer;
        IVault vault;
    }

    /// @dev Retrieves the current effective balance for use with CoW Protocol.
    ///
    /// @param contracts - On-chain contract addresses required for the
    /// simulation.
    /// @param trader - The address of the account intending to trade on CoW
    /// Protocol.
    /// @param token - The address of the sell token of the trader.
    /// @param amount - The amount of tokens to attempt to transfer. If this
    /// value is specfied as 0, then the entire effective balance will be
    /// transferred.
    /// @param source - The balance source, this can either be the Keccak-256
    /// digest of "erc20", "external", or "internal". This corresponds to the
    /// `sellTokenBalance` field of the CoW Protocol order.
    /// @param interactions - A list of pre-interactions required for setting
    /// up balances and/or allowances.
    ///
    /// @return tokenBalance - The token balance of the user.
    /// @return allowance - The allowance set by the user to the Protocol.
    /// @return effectiveBalance - The effective balance of the user. This
    /// represents a balance that can be transferred into the settlement
    /// contract when executing a trade.
    /// @return canTransfer - Returns whether or not the transfer into the
    /// settlement contract for the specified amount would succeed.
    function balance(
        Contracts memory contracts,
        address trader,
        IERC20 token,
        uint256 amount,
        bytes32 source,
        Interaction[] calldata interactions
    ) external returns (
        uint256 tokenBalance,
        uint256 allowance,
        uint256 effectiveBalance,
        bool canTransfer
    ) {
        // Execute the interactions within the current context. This ensures
        // that any pre-interactions that setup balances and/or allowances
        // are executed before reading them.
        executeInteractions(contracts, interactions);

        // Read the traders token balance and allowance.
        if (source == keccak256("erc20")) {
            tokenBalance = token.balanceOf(trader);
            allowance = token.allowance(trader, address(contracts.vaultRelayer));
        } else if (source == keccak256("external")) {
            tokenBalance = token.balanceOf(trader);
            allowance = contracts.vault
                .hasApprovedRelayer(trader, address(contracts.vaultRelayer))
                    ? token.allowance(trader, address(contracts.vault))
                    : 0;
        } else if (source == keccak256("internal")) {
            address[] memory tokens = new address[](1);
            tokens[0] = address(token);
            tokenBalance = contracts.vault.getInternalBalance(trader, tokens)[0];
            allowance = contracts.vault
                .hasApprovedRelayer(trader, address(contracts.vaultRelayer))
                    ? type(uint256).max
                    : 0;
        } else {
            revert("invalid token source");
        }

        effectiveBalance = tokenBalance <= allowance
            ? tokenBalance
            : allowance;

        // Verify that the transfer for the complete effective balance actually
        // works.
        Transfer[] memory transfers = new Transfer[](1);
        transfers[0] = Transfer({
            account: trader,
            token: address(token),
            amount: amount != 0
                ? amount
                : effectiveBalance,
            balance: source
        });
        try contracts.vaultRelayer.transferFromAccounts(transfers) {
            canTransfer = true;
        }
        catch {
            canTransfer = false;
        }
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

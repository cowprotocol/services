// SPDX-License-Identifier: LGPL-3.0-or-later
pragma solidity ^0.7.6;
pragma abicoder v2;

import "./interfaces/IERC20.sol";
import "./interfaces/IVault.sol";
import "./libraries/GPv2Transfer.sol";

/// @title Gnosis Protocol v2 Vault Relayer Contract
/// @author Gnosis Developers
contract GPv2VaultRelayer {
    using GPv2Transfer for IVault;

    /// @dev The creator of the contract which has special permissions. This
    /// value is set at creation time and cannot change.
    address private immutable creator;

    /// @dev The vault this relayer is for.
    IVault private immutable vault;

    constructor(IVault vault_) {
        creator = msg.sender;
        vault = vault_;
    }

    /// @dev Modifier that ensures that a function can only be called by the
    /// creator of this contract.
    modifier onlyCreator() {
        require(msg.sender == creator, "GPv2: not creator");
        _;
    }

    /// @dev Transfers all sell amounts for the executed trades from their
    /// owners to the caller.
    ///
    /// This function reverts if:
    /// - The caller is not the creator of the vault relayer
    /// - Any ERC20 transfer fails
    ///
    /// @param transfers The transfers to execute.
    function transferFromAccounts(GPv2Transfer.Data[] calldata transfers)
        external
        onlyCreator
    {
        vault.transferFromAccounts(transfers, msg.sender);
    }

    /// @dev Performs a Balancer batched swap on behalf of a user and sends a
    /// fee to the caller.
    ///
    /// This function reverts if:
    /// - The caller is not the creator of the vault relayer
    /// - The swap fails
    /// - The fee transfer fails
    ///
    /// @param kind The Balancer swap kind, this can either be `GIVEN_IN` for
    /// sell orders or `GIVEN_OUT` for buy orders.
    /// @param swaps The swaps to perform.
    /// @param tokens The tokens for the swaps. Swaps encode to and from tokens
    /// as indices into this array.
    /// @param funds The fund management settings, specifying the user the swap
    /// is being performed for as well as the recipient of the proceeds.
    /// @param limits Swap limits for encoding limit prices.
    /// @param deadline The deadline for the swap.
    /// @param feeTransfer The transfer data for the caller fee.
    /// @return tokenDeltas The executed swap amounts.
    function batchSwapWithFee(
        IVault.SwapKind kind,
        IVault.BatchSwapStep[] calldata swaps,
        IERC20[] memory tokens,
        IVault.FundManagement memory funds,
        int256[] memory limits,
        uint256 deadline,
        GPv2Transfer.Data calldata feeTransfer
    ) external onlyCreator returns (int256[] memory tokenDeltas) {
        tokenDeltas = vault.batchSwap(
            kind,
            swaps,
            tokens,
            funds,
            limits,
            deadline
        );
        vault.fastTransferFromAccount(feeTransfer, msg.sender);
    }
}

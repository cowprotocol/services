// SPDX-License-Identifier: LGPL-3.0-or-later
pragma solidity ^0.7.6;
pragma abicoder v2;

import "../interfaces/IERC20.sol";
import "../interfaces/IVault.sol";
import "./GPv2Order.sol";
import "./GPv2SafeERC20.sol";

/// @title Gnosis Protocol v2 Transfers
/// @author Gnosis Developers
library GPv2Transfer {
    using GPv2SafeERC20 for IERC20;

    /// @dev Transfer data.
    struct Data {
        address account;
        IERC20 token;
        uint256 amount;
        bytes32 balance;
    }

    /// @dev Ether marker address used to indicate an Ether transfer.
    address internal constant BUY_ETH_ADDRESS =
        0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE;

    /// @dev Execute the specified transfer from the specified account to a
    /// recipient. The recipient will either receive internal Vault balances or
    /// ERC20 token balances depending on whether the account is using internal
    /// balances or not.
    ///
    /// This method is used for transferring fees to the settlement contract
    /// when settling a single order directly with Balancer.
    ///
    /// Note that this method is subtly different from `transferFromAccounts`
    /// with a single transfer with respect to how it deals with internal
    /// balances. Specifically, this method will perform an **internal balance
    /// transfer to the settlement contract instead of a withdrawal to the
    /// external balance of the settlement contract** for trades that specify
    /// trading with internal balances. This is done as a gas optimization in
    /// the single order "fast-path".
    ///
    /// @param vault The Balancer vault to use.
    /// @param transfer The transfer to perform specifying the sender account.
    /// @param recipient The recipient for the transfer.
    function fastTransferFromAccount(
        IVault vault,
        Data calldata transfer,
        address recipient
    ) internal {
        require(
            address(transfer.token) != BUY_ETH_ADDRESS,
            "GPv2: cannot transfer native ETH"
        );

        if (transfer.balance == GPv2Order.BALANCE_ERC20) {
            transfer.token.safeTransferFrom(
                transfer.account,
                recipient,
                transfer.amount
            );
        } else {
            IVault.UserBalanceOp[]
                memory balanceOps = new IVault.UserBalanceOp[](1);

            IVault.UserBalanceOp memory balanceOp = balanceOps[0];
            balanceOp.kind = transfer.balance == GPv2Order.BALANCE_EXTERNAL
                ? IVault.UserBalanceOpKind.TRANSFER_EXTERNAL
                : IVault.UserBalanceOpKind.TRANSFER_INTERNAL;
            balanceOp.asset = transfer.token;
            balanceOp.amount = transfer.amount;
            balanceOp.sender = transfer.account;
            balanceOp.recipient = payable(recipient);

            vault.manageUserBalance(balanceOps);
        }
    }

    /// @dev Execute the specified transfers from the specified accounts to a
    /// single recipient. The recipient will receive all transfers as ERC20
    /// token balances, regardless of whether or not the accounts are using
    /// internal Vault balances.
    ///
    /// This method is used for accumulating user balances into the settlement
    /// contract.
    ///
    /// @param vault The Balancer vault to use.
    /// @param transfers The batched transfers to perform specifying the
    /// sender accounts.
    /// @param recipient The single recipient for all the transfers.
    function transferFromAccounts(
        IVault vault,
        Data[] calldata transfers,
        address recipient
    ) internal {
        // NOTE: Allocate buffer of Vault balance operations large enough to
        // hold all GP transfers. This is done to avoid re-allocations (which
        // are gas inefficient) while still allowing all transfers to be batched
        // into a single Vault call.
        IVault.UserBalanceOp[] memory balanceOps = new IVault.UserBalanceOp[](
            transfers.length
        );
        uint256 balanceOpCount = 0;

        for (uint256 i = 0; i < transfers.length; i++) {
            Data calldata transfer = transfers[i];
            require(
                address(transfer.token) != BUY_ETH_ADDRESS,
                "GPv2: cannot transfer native ETH"
            );

            if (transfer.balance == GPv2Order.BALANCE_ERC20) {
                transfer.token.safeTransferFrom(
                    transfer.account,
                    recipient,
                    transfer.amount
                );
            } else {
                IVault.UserBalanceOp memory balanceOp = balanceOps[
                    balanceOpCount++
                ];
                balanceOp.kind = transfer.balance == GPv2Order.BALANCE_EXTERNAL
                    ? IVault.UserBalanceOpKind.TRANSFER_EXTERNAL
                    : IVault.UserBalanceOpKind.WITHDRAW_INTERNAL;
                balanceOp.asset = transfer.token;
                balanceOp.amount = transfer.amount;
                balanceOp.sender = transfer.account;
                balanceOp.recipient = payable(recipient);
            }
        }

        if (balanceOpCount > 0) {
            truncateBalanceOpsArray(balanceOps, balanceOpCount);
            vault.manageUserBalance(balanceOps);
        }
    }

    /// @dev Execute the specified transfers to their respective accounts.
    ///
    /// This method is used for paying out trade proceeds from the settlement
    /// contract.
    ///
    /// @param vault The Balancer vault to use.
    /// @param transfers The batched transfers to perform.
    function transferToAccounts(IVault vault, Data[] memory transfers)
        internal
    {
        IVault.UserBalanceOp[] memory balanceOps = new IVault.UserBalanceOp[](
            transfers.length
        );
        uint256 balanceOpCount = 0;

        for (uint256 i = 0; i < transfers.length; i++) {
            Data memory transfer = transfers[i];

            if (address(transfer.token) == BUY_ETH_ADDRESS) {
                require(
                    transfer.balance != GPv2Order.BALANCE_INTERNAL,
                    "GPv2: unsupported internal ETH"
                );
                payable(transfer.account).transfer(transfer.amount);
            } else if (transfer.balance == GPv2Order.BALANCE_ERC20) {
                transfer.token.safeTransfer(transfer.account, transfer.amount);
            } else {
                IVault.UserBalanceOp memory balanceOp = balanceOps[
                    balanceOpCount++
                ];
                balanceOp.kind = IVault.UserBalanceOpKind.DEPOSIT_INTERNAL;
                balanceOp.asset = transfer.token;
                balanceOp.amount = transfer.amount;
                balanceOp.sender = address(this);
                balanceOp.recipient = payable(transfer.account);
            }
        }

        if (balanceOpCount > 0) {
            truncateBalanceOpsArray(balanceOps, balanceOpCount);
            vault.manageUserBalance(balanceOps);
        }
    }

    /// @dev Truncate a Vault balance operation array to its actual size.
    ///
    /// This method **does not** check whether or not the new length is valid,
    /// and specifying a size that is larger than the array's actual length is
    /// undefined behaviour.
    ///
    /// @param balanceOps The memory array of balance operations to truncate.
    /// @param newLength The new length to set.
    function truncateBalanceOpsArray(
        IVault.UserBalanceOp[] memory balanceOps,
        uint256 newLength
    ) private pure {
        // NOTE: Truncate the vault transfers array to the specified length.
        // This is done by setting the array's length which occupies the first
        // word in memory pointed to by the `balanceOps` memory variable.
        // <https://docs.soliditylang.org/en/v0.7.6/internals/layout_in_memory.html>
        // solhint-disable-next-line no-inline-assembly
        assembly {
            mstore(balanceOps, newLength)
        }
    }
}

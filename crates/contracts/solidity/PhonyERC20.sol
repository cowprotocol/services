// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { Caller } from "./libraries/Caller.sol";
import { SafeERC20 } from "./libraries/SafeERC20.sol";

/// @dev A phony ERC20 implementation, that replaces the bytecode of an existing
/// on-chain contract and allows minting to arbitrary addresses. This can be
/// used to fund an account when one with a known balance can't be found.
contract PhonyERC20 {
    using Caller for *;
    using SafeERC20 for *;

    /// @dev A seed to offset all internal balance storage operations to make
    /// sure that we don't accidentally touch any of the implementation's slots.
    /// Derived from:
    /// ```
    /// keccak("hakuna matata") - 1
    /// ```
    ///
    /// Note that we subtract 1 from the hash so that their is no known
    /// pre-image, so hash collisions are not possible.
    uint256 constant private BALANCES_SLOT =
        0x2dc49bd971a218a45c433d8da1ecae9b9e80fb7d8335e0369a90da5010750285;

    /// @dev Address where the original code for the token implementation is
    /// expected to be. Use 0x10000 as its the first "valid" address, since
    /// addresses up to 0xffff are reserved for pre-compiles.
    address constant private IMPLEMENTATION = address(0x10000);

    event Transfer(address indexed from, address indexed to, uint256 value);

    // Make sure to forward all remaining calls to the actual ERC20
    // implementation.
    fallback() external payable {
        _fallback();
    }
    receive() external payable {
        _fallback();
    }

    /// @dev Returns the balance of the specified address. This is the sum of
    /// the implementation ERC20 balance and the internal balance.
    function balanceOf(address owner) external returns (uint256) {
        uint256 implementationBalance = _implementationBalanceOf(owner);
        uint256 internalBalance = _balancesSlot()[owner];

        return implementationBalance + internalBalance;
    }

    /// @dev Transfer tokens from `msg.sender` to the specified `to` address.
    /// This function will prefer transferring the implementation ERC20 balance
    /// and fallback to using internal balances if needed. This way, we make
    /// sure to call the implementation's `transfer` function whenever possible,
    /// for as much token as possible.
    function transfer(address to, uint256 value) external returns (bool) {
        uint256 realAmount = _transferExcessInternally(msg.sender, to, value);

        if (realAmount > 0) {
            IMPLEMENTATION.doDelegatecall(abi.encodeCall(this.transfer, (to, realAmount)))
                .check("PhonyERC20: transfer failed");
        }

        return true;
    }

    /// @dev Transfer tokens from the specified `from` address to the specified
    /// `to` address. Like `transfer`, this function will prefer transferring
    /// the implementation ERC20 balance and fallback to using internal balances
    /// if needed.
    function transferFrom(address from, address to, uint256 value) external returns (bool) {
        uint256 realAmount = _transferExcessInternally(from, to, value);

        if (realAmount > 0) {
            IMPLEMENTATION.doDelegatecall(abi.encodeCall(this.transferFrom, (from, to, realAmount)))
                .check("PhonyERC20: transferFrom failed");
        }

        return true;
    }

    function mintPhonyTokens(address receiver, uint256 amount) external returns (bool) {
        _balancesSlot()[receiver] += amount;
        return true;
    }

    function _fallback() private {
        bytes memory rdata = IMPLEMENTATION.doDelegatecall(msg.data);
        assembly { return(add(rdata, 32), mload(rdata)) }
    }

    /// @dev Get the storage slot used for storing internal account balances.
    function _balancesSlot() private pure returns (
        mapping(address => uint256) storage balances
    ) {
        uint256 slot = BALANCES_SLOT;
        assembly { balances.slot := slot }
    }

    /// @dev Get the implementation ERC20 balance for the specified account.
    function _implementationBalanceOf(address owner) private returns (uint256) {
        return abi.decode(
            IMPLEMENTATION.doDelegatecall(abi.encodeCall(this.balanceOf, (owner))),
            (uint256)
        );
    }

    /// @dev Compute the largest real (i.e. implementation) transfer amount
    /// possible and the corresponding remaining internal balance transfer.
    /// Additionally, execute the internal balance transfer and return the
    /// remaining real transfer amount that still needs to be done.
    ///
    /// This is a shared function used in both `transfer` and `transferFrom`
    /// where they both want to transfer the maximum amount of implementation
    /// balance possible, but need to do so with different implementation
    /// transfer functions.
    function _transferExcessInternally(
        address from,
        address to,
        uint256 value
    ) private returns (
        uint256 realAmount
    ) {
        uint256 implementationBalance = _implementationBalanceOf(from);
        uint256 internalAmount = implementationBalance < value
            ? value - implementationBalance
            : 0;

        if (internalAmount > 0) {
            mapping(address => uint256) storage balances = _balancesSlot();
            balances[from] -= internalAmount;
            balances[to] += internalAmount;

            emit Transfer(from, to, internalAmount);
        }

        realAmount = value - internalAmount;
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "./NonStandardERC20Balances.sol";

contract RemoteERC20Balances is NonStandardERC20Balances {
    bool internal immutable isLocalBalance;
    NonStandardERC20Balances public immutable target;

    constructor(NonStandardERC20Balances _target, bool _balanceFromHere) {
        isLocalBalance = _balanceFromHere;
        target = _target;
    }

    function balanceOf(address user) external view override returns (uint256) {
        // retrieve the balance from the target contract regardless (for testing)
        uint256 otherBalanceOf = target.balanceOf(user);
        
        return isLocalBalance ? users[user].balance : otherBalanceOf;
    }

}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

contract NonStandardERC20Balances {

    struct UserData {
        // hypothetical other field
        uint256 epoch;
        mapping(address => uint256) approvals;
        uint256 balance;
    }

    // In this token, the user's balance is stored at an offset position in the mapping -> cannot be detected by standard methods
    mapping(address => UserData) users;

    function mint(address user, uint256 amount) external {
        users[user].epoch = 1;
        users[user].balance = amount;
    }

    function balanceOf(address user) external virtual view returns (uint256) {
        return users[user].balance;
    }

    function allowance(address user, address spender) external view returns (uint256) {
        return users[user].approvals[spender];
    }

    function transfer(address to, uint256 amount) external {
        users[msg.sender].balance -= amount;
        users[msg.sender].epoch++;
        users[to].balance += amount;
        users[to].epoch++;
    }

    function transferFrom(address from, address to, uint256 amount) external {
        users[from].approvals[msg.sender] -= amount;
        users[from].balance -= amount;
        users[from].epoch++;
        users[to].balance += amount;
        users[to].epoch++;
    }

    function approve(address spender, uint256 amount) external {
        users[msg.sender].approvals[spender] = amount;
    }
}

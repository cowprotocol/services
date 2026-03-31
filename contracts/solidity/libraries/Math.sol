// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

library Math {
    function toInt(uint256 self) internal pure returns (int256) {
        require(self <= uint256(type(int256).max), "Math: int256 overflow");
        return int256(self);
    }

    function check(bytes memory self, string memory message) internal pure {
        require(self.length == 0 || abi.decode(self, (bool)), message);
    }
}

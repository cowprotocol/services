// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IERC20 } from "../interfaces/IERC20.sol";
import { Caller } from "./Caller.sol";

library SafeERC20 {
    using Caller for *;

    function safeTransfer(IERC20 self, address target, uint256 amount) internal {
        bytes memory cdata = abi.encodeCall(self.transfer, (target, amount));
        bytes memory rdata = address(self).doCall(cdata);
        require(check(rdata), "SafeERC20: transfer failed");
    }

    function safeApprove(IERC20 self, address target, uint256 amount) internal {
        bytes memory cdata = abi.encodeCall(self.approve, (target, amount));
        bytes memory rdata = address(self).doCall(cdata);
        require(check(rdata), "SafeERC20: approval failed");
    }

    function check(bytes memory rdata) internal pure returns (bool ok) {
        return rdata.length == 0 || abi.decode(rdata, (bool));
    }
}

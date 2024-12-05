// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IERC20 } from "../interfaces/IERC20.sol";
import { Caller } from "./Caller.sol";

library SafeERC20 {
    using Caller for *;

    function trySafeTransfer(IERC20 self, address target, uint256 amount) internal returns (bool success) {
        bytes memory cdata = abi.encodeCall(self.transfer, (target, amount));
        bytes memory rdata;
        (success, rdata) = address(self).call(cdata);
        return success && check(rdata);
    }

    function safeApprove(IERC20 self, address target, uint256 amount) internal {
        bytes memory cdata = abi.encodeCall(self.approve, (target, amount));
        bytes memory rdata = address(self).doCall(cdata);
        ensure(rdata, "SafeERC20: approval failed");
    }

    function check(bytes memory rdata) internal pure returns (bool ok) {
        return rdata.length == 0 || abi.decode(rdata, (bool));
    }

    function ensure(bytes memory rdata, string memory message) internal pure {
        require(check(rdata), message);
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.16;

import { IERC20 } from "../interfaces/IERC20.sol";
import { Caller } from "./Caller.sol";

library SafeERC20 {
    using Caller for *;

    function safeApprove(IERC20 self, address target, uint256 amount) internal {
        bytes memory cdata = abi.encodeCall(self.approve, (target, amount));
        bytes memory rdata = address(self).doCall(cdata);
        check(rdata, "SafeERC20: approval failed");
    }

    function check(bytes memory self, string memory message) internal pure {
        require(self.length == 0 || abi.decode(self, (bool)), message);
    }
}

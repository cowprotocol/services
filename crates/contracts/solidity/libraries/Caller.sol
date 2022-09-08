// SPDX-License-Identifier: MIT
pragma solidity ^0.8.16;

import { Interaction } from "../interfaces/ISettlement.sol";

library Caller {
    function doCall(address self, bytes memory cdata) internal returns (bytes memory rdata) {
        rdata = doCall(self, 0, cdata);
    }

    function doCall(address self, uint256 value, bytes memory cdata) internal returns (bytes memory rdata) {
        bool success;
        (success, rdata) = self.call{value: value}(cdata);
        if (!success) {
            assembly { revert(add(rdata, 32), mload(rdata)) }
        }
    }

    function doDelegatecall(address self, bytes memory cdata) internal returns (bytes memory rdata) {
        bool success;
        (success, rdata) = self.delegatecall(cdata);
        if (!success) {
            assembly { revert(add(rdata, 32), mload(rdata)) }
        }
    }

    function doMeteredCallNoReturn(address self, bytes memory cdata) internal returns (uint256 gasUsed) {
        gasUsed = doMeteredCallNoReturn(self, 0, cdata);
    }

    function doMeteredCallNoReturn(address self, uint256 value, bytes memory cdata) internal returns (uint256 gasUsed) {
        gasUsed = gasleft();
        assembly {
            if iszero(call(gas(), self, value, add(cdata, 32), mload(cdata), 0, 0)) {
                returndatacopy(0, 0, returndatasize())
                revert(0, returndatasize())
            }
        }
        gasUsed -= gasleft();
    }
}


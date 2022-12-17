// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @title A contract for executing multiple calls withing a single calling
/// context.
///
/// This contract is designed to work on all networks and not require any
/// existing contract deployment.
contract Multicall {
    struct Call {
        address to;
        uint256 gas;
        uint256 value;
        bytes data;
    }

    struct Result {
        bool success;
        bytes data;
    }

    constructor(Call[] memory calls) payable {
        Trampoline trampoline = new Trampoline();

        Result[] memory results = new Result[](calls.length);
        unchecked {
            for (uint256 i = 0; i < calls.length; i++) {
                Call memory call = calls[i];
                Result memory result = results[i];

                try trampoline.execute{value: call.value}(
                    call.to,
                    call.gas,
                    call.data
                ) {
                    revert("unreachable");
                } catch (bytes memory resultData) {
                    (
                        result.success,
                        result.data
                    ) = abi.decode(resultData, (bool, bytes));
                }
            }
        }

        bytes memory returnData = abi.encode((results));
        assembly {
            return(add(32, returnData), mload(returnData))
        }
    }
}

/// @title Multicall trampoline to ensure that we always undo the work of the
/// previous call before starting the next one, ensuring that subsequent calls
/// do not affect eachother.
contract Trampoline {
    function execute(address to, uint256 gas, bytes calldata data) public payable {
        if (gas == 0) {
            gas = gasleft();
        }

        (bool success, bytes memory returnData) =
            to.call{value: msg.value, gas: gas}(data);

        bytes memory resultData = abi.encode(success, returnData);
        assembly {
            revert(add(32, resultData), mload(resultData))
        }
    }
}

// SPDX-License-Identifier: LGPL-3.0-only

// Vendored from Gnosis utility contracts with minor modifications:
// - Modified Solidity version
// - Formatted code
// - Added linter directives to ignore low level call and assembly warnings
// <https://github.com/gnosis/util-contracts/blob/v3.1.0-solc-7/contracts/StorageAccessible.sol>

pragma solidity ^0.7.6;

/// @title ViewStorageAccessible - Interface on top of StorageAccessible base class to allow simulations from view functions
interface ViewStorageAccessible {
    /**
     * @dev Same as `simulateDelegatecall` on StorageAccessible. Marked as view so that it can be called from external contracts
     * that want to run simulations from within view functions. Will revert if the invoked simulation attempts to change state.
     */
    function simulateDelegatecall(
        address targetContract,
        bytes memory calldataPayload
    ) external view returns (bytes memory);

    /**
     * @dev Same as `getStorageAt` on StorageAccessible. This method allows reading aribtrary ranges of storage.
     */
    function getStorageAt(uint256 offset, uint256 length)
        external
        view
        returns (bytes memory);
}

/// @title StorageAccessible - generic base contract that allows callers to access all internal storage.
contract StorageAccessible {
    /**
     * @dev Reads `length` bytes of storage in the currents contract
     * @param offset - the offset in the current contract's storage in words to start reading from
     * @param length - the number of words (32 bytes) of data to read
     * @return the bytes that were read.
     */
    function getStorageAt(uint256 offset, uint256 length)
        external
        view
        returns (bytes memory)
    {
        bytes memory result = new bytes(length * 32);
        for (uint256 index = 0; index < length; index++) {
            // solhint-disable-next-line no-inline-assembly
            assembly {
                let word := sload(add(offset, index))
                mstore(add(add(result, 0x20), mul(index, 0x20)), word)
            }
        }
        return result;
    }

    /**
     * @dev Performs a delegetecall on a targetContract in the context of self.
     * Internally reverts execution to avoid side effects (making it static). Catches revert and returns encoded result as bytes.
     * @param targetContract Address of the contract containing the code to execute.
     * @param calldataPayload Calldata that should be sent to the target contract (encoded method name and arguments).
     */
    function simulateDelegatecall(
        address targetContract,
        bytes memory calldataPayload
    ) public returns (bytes memory response) {
        bytes memory innerCall = abi.encodeWithSelector(
            this.simulateDelegatecallInternal.selector,
            targetContract,
            calldataPayload
        );
        // solhint-disable-next-line avoid-low-level-calls
        (, response) = address(this).call(innerCall);
        bool innerSuccess = response[response.length - 1] == 0x01;
        setLength(response, response.length - 1);
        if (innerSuccess) {
            return response;
        } else {
            revertWith(response);
        }
    }

    /**
     * @dev Performs a delegetecall on a targetContract in the context of self.
     * Internally reverts execution to avoid side effects (making it static). Returns encoded result as revert message
     * concatenated with the success flag of the inner call as a last byte.
     * @param targetContract Address of the contract containing the code to execute.
     * @param calldataPayload Calldata that should be sent to the target contract (encoded method name and arguments).
     */
    function simulateDelegatecallInternal(
        address targetContract,
        bytes memory calldataPayload
    ) external returns (bytes memory response) {
        bool success;
        // solhint-disable-next-line avoid-low-level-calls
        (success, response) = targetContract.delegatecall(calldataPayload);
        revertWith(abi.encodePacked(response, success));
    }

    function revertWith(bytes memory response) internal pure {
        // solhint-disable-next-line no-inline-assembly
        assembly {
            revert(add(response, 0x20), mload(response))
        }
    }

    function setLength(bytes memory buffer, uint256 length) internal pure {
        // solhint-disable-next-line no-inline-assembly
        assembly {
            mstore(buffer, length)
        }
    }
}

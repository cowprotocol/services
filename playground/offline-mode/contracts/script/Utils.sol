// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// @title DeploymentUtils
/// @notice Utilities for deterministic contract deployments using CREATE2
library DeploymentUtils {
    /// @dev Deploy contract using CREATE2 for deterministic addresses
    /// @param bytecode The contract bytecode (including constructor arguments if any)
    /// @param salt The salt for CREATE2 deployment
    /// @return addr The deployed contract address
    function deployWithCreate2(bytes memory bytecode, bytes32 salt) internal returns (address addr) {
        assembly {
            addr := create2(0, add(bytecode, 0x20), mload(bytecode), salt)
            if iszero(extcodesize(addr)) {
                revert(0, 0)
            }
        }
    }

    /// @dev Compute the CREATE2 address for a given bytecode and salt
    /// @param bytecode The contract bytecode (including constructor arguments if any)
    /// @param salt The salt for CREATE2 deployment
    /// @param deployer The address that will deploy the contract
    /// @return The predicted contract address
    function computeCreate2Address(
        bytes memory bytecode,
        bytes32 salt,
        address deployer
    ) internal pure returns (address) {
        bytes32 bytecodeHash = keccak256(bytecode);
        bytes32 hash = keccak256(
            abi.encodePacked(
                bytes1(0xff),
                deployer,
                salt,
                bytecodeHash
            )
        );
        return address(uint160(uint256(hash)));
    }
}

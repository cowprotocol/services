// SPDX-License-Identifier: LGPL-3.0-or-later
pragma solidity ^0.7.6;

library GPv2EIP1271 {
    /// @dev Value returned by a call to `isValidSignature` if the signature
    /// was verified successfully. The value is defined in EIP-1271 as:
    /// bytes4(keccak256("isValidSignature(bytes32,bytes)"))
    bytes4 internal constant MAGICVALUE = 0x1626ba7e;
}

/// @title EIP1271 Interface
/// @dev Standardized interface for an implementation of smart contract
/// signatures as described in EIP-1271. The code that follows is identical to
/// the code in the standard with the exception of formatting and syntax
/// changes to adapt the code to our Solidity version.
interface EIP1271Verifier {
    /// @dev Should return whether the signature provided is valid for the
    /// provided data
    /// @param _hash      Hash of the data to be signed
    /// @param _signature Signature byte array associated with _data
    ///
    /// MUST return the bytes4 magic value 0x1626ba7e when function passes.
    /// MUST NOT modify state (using STATICCALL for solc < 0.5, view modifier for
    /// solc > 0.5)
    /// MUST allow external calls
    ///
    function isValidSignature(bytes32 _hash, bytes memory _signature)
        external
        view
        returns (bytes4 magicValue);
}

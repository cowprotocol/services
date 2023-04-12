// SPDX-License-Identifier: LGPL-3.0-or-later
pragma solidity ^0.7.6;

library GPv2EIP1967 {
    /// @dev The storage slot where the proxy administrator is stored, defined
    /// as `keccak256('eip1967.proxy.admin') - 1`.
    bytes32 internal constant ADMIN_SLOT =
        hex"b53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";

    /// @dev Returns the address stored in the EIP-1967 administrator storage
    /// slot for the current contract. If this method is not called from an
    /// contract behind an EIP-1967 proxy, then it will most likely return
    /// `address(0)`, as the implementation slot is likely to be unset.
    ///
    /// @return admin The administrator address.
    function getAdmin() internal view returns (address admin) {
        // solhint-disable-next-line no-inline-assembly
        assembly {
            admin := sload(ADMIN_SLOT)
        }
    }

    /// @dev Sets the storage at the EIP-1967 administrator slot to be the
    /// specified address.
    ///
    /// @param admin The administrator address to set.
    function setAdmin(address admin) internal {
        // solhint-disable-next-line no-inline-assembly
        assembly {
            sstore(ADMIN_SLOT, admin)
        }
    }
}

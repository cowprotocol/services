// SPDX-License-Identifier: LGPL-3.0-or-later
pragma solidity ^0.7.6;

import "./interfaces/GPv2Authentication.sol";
import "./libraries/GPv2EIP1967.sol";
import "./mixins/Initializable.sol";
import "./mixins/StorageAccessible.sol";

/// @title Gnosis Protocol v2 Access Control Contract
/// @author Gnosis Developers
contract GPv2AllowListAuthentication is
    GPv2Authentication,
    Initializable,
    StorageAccessible
{
    /// @dev The address of the manager that has permissions to add and remove
    /// solvers.
    address public manager;

    /// @dev The set of allowed solvers. Allowed solvers have a value of `true`
    /// in this mapping.
    mapping(address => bool) private solvers;

    /// @dev Event emitted when the manager changes.
    event ManagerChanged(address newManager, address oldManager);

    /// @dev Event emitted when a solver gets added.
    event SolverAdded(address solver);

    /// @dev Event emitted when a solver gets removed.
    event SolverRemoved(address solver);

    /// @dev Initialize the manager to a value.
    ///
    /// This method is a contract initializer that is called exactly once after
    /// creation. An initializer is used instead of a constructor so that this
    /// contract can be used behind a proxy.
    ///
    /// This initializer is idempotent.
    ///
    /// @param manager_ The manager to initialize the contract with.
    function initializeManager(address manager_) external initializer {
        manager = manager_;
        emit ManagerChanged(manager_, address(0));
    }

    /// @dev Modifier that ensures a method can only be called by the contract
    /// manager. Reverts if called by other addresses.
    modifier onlyManager() {
        require(manager == msg.sender, "GPv2: caller not manager");
        _;
    }

    /// @dev Modifier that ensures method can be either called by the contract
    /// manager or the proxy owner.
    ///
    /// This modifier assumes that the proxy uses an EIP-1967 compliant storage
    /// slot for the admin.
    modifier onlyManagerOrOwner() {
        require(
            manager == msg.sender || GPv2EIP1967.getAdmin() == msg.sender,
            "GPv2: not authorized"
        );
        _;
    }

    /// @dev Set the manager for this contract.
    ///
    /// This method can be called by the current manager (if they want to to
    /// reliquish the role and give it to another address) or the contract
    /// owner (i.e. the proxy admin).
    ///
    /// @param manager_ The new contract manager address.
    function setManager(address manager_) external onlyManagerOrOwner {
        address oldManager = manager;
        manager = manager_;
        emit ManagerChanged(manager_, oldManager);
    }

    /// @dev Add an address to the set of allowed solvers. This method can only
    /// be called by the contract manager.
    ///
    /// This function is idempotent.
    ///
    /// @param solver The solver address to add.
    function addSolver(address solver) external onlyManager {
        solvers[solver] = true;
        emit SolverAdded(solver);
    }

    /// @dev Removes an address to the set of allowed solvers. This method can
    /// only be called by the contract manager.
    ///
    /// This function is idempotent.
    ///
    /// @param solver The solver address to remove.
    function removeSolver(address solver) external onlyManager {
        solvers[solver] = false;
        emit SolverRemoved(solver);
    }

    /// @inheritdoc GPv2Authentication
    function isSolver(address prospectiveSolver)
        external
        view
        override
        returns (bool)
    {
        return solvers[prospectiveSolver];
    }
}

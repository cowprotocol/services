// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// @title MockBalancerVault
/// @notice Minimal mock of Balancer V2 Vault for CoW Protocol compatibility
/// @dev Only implements methods required for GPv2Settlement to work
contract MockBalancerVault {
    // Pool registry: poolId => pool address
    mapping(bytes32 => address) public pools;
    
    // Token registry: poolId => token addresses
    mapping(bytes32 => address[]) public poolTokens;

    event PoolRegistered(bytes32 indexed poolId, address indexed pool, address[] tokens);

    /// @notice Register a pool (for testing)
    function registerPool(
        bytes32 poolId,
        address pool,
        address[] calldata tokens
    ) external {
        pools[poolId] = pool;
        poolTokens[poolId] = tokens;
        emit PoolRegistered(poolId, pool, tokens);
    }

    /// @notice Get pool address by ID
    function getPool(bytes32 poolId) external view returns (address) {
        return pools[poolId];
    }

    /// @notice Get pool tokens
    function getPoolTokens(bytes32 poolId)
        external
        view
        returns (
            address[] memory tokens,
            uint256[] memory balances,
            uint256 lastChangeBlock
        )
    {
        tokens = poolTokens[poolId];
        balances = new uint256[](tokens.length);
        lastChangeBlock = block.number;
    }

    /// @notice Mock batch swap (not implemented - CoW uses BALANCE_ERC20 mode)
    function batchSwap(
        uint8, // kind
        bytes32[] calldata, // swaps
        address[] calldata, // assets
        bytes calldata, // funds
        int256[] calldata, // limits
        uint256 // deadline
    ) external pure returns (int256[] memory) {
        revert("MockBalancerVault: batchSwap not implemented - use BALANCE_ERC20");
    }

    /// @notice Mock manageUserBalance (not implemented)
    function manageUserBalance(bytes32[] calldata) external pure {
        revert("MockBalancerVault: manageUserBalance not implemented");
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8.17;

import "./MockUniswapV3Pool.sol";

/// @title Minimal mock of a Uniswap V3 factory for indexer e2e tests.
/// @dev `createPool` deploys a `MockUniswapV3Pool` and emits the same
///      `PoolCreated` event the pool-indexer listens for.
contract MockUniswapV3Factory {
    event PoolCreated(
        address indexed token0,
        address indexed token1,
        uint24  indexed fee,
        int24           tickSpacing,
        address         pool
    );

    function createPool(
        address tokenA,
        address tokenB,
        uint24  _fee
    ) external returns (address pool) {
        (address t0, address t1) = tokenA < tokenB
            ? (tokenA, tokenB)
            : (tokenB, tokenA);

        MockUniswapV3Pool p = new MockUniswapV3Pool(t0, t1, _fee);
        pool = address(p);

        emit PoolCreated(t0, t1, _fee, int24(10), pool);
    }
}

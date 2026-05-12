// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8.17;

/// @title Minimal mock of a Uniswap V3 pool for indexer e2e tests.
/// @dev Emits the same events the pool-indexer listens for.  Only the
///      subset of state that the indexer actually reads is stored.
contract MockUniswapV3Pool {
    address public immutable token0;
    address public immutable token1;
    uint24  public immutable fee;

    uint128 public liquidity;

    event Initialize(uint160 sqrtPriceX96, int24 tick);

    event Mint(
        address          sender,
        address indexed  owner,
        int24   indexed  tickLower,
        int24   indexed  tickUpper,
        uint128          amount,
        uint256          amount0,
        uint256          amount1
    );

    constructor(address _token0, address _token1, uint24 _fee) {
        token0 = _token0;
        token1 = _token1;
        fee    = _fee;
    }

    function initialize(uint160 sqrtPriceX96) external {
        emit Initialize(sqrtPriceX96, int24(0));
    }

    function mockMint(
        address owner,
        int24   tickLower,
        int24   tickUpper,
        uint128 amount
    ) external {
        liquidity += amount;
        emit Mint(msg.sender, owner, tickLower, tickUpper, amount, 0, 0);
    }
}

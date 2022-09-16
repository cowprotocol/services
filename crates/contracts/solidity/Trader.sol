// SPDX-License-Identifier: MIT
pragma solidity ^0.8.16;

import { IERC20, INativeERC20 } from "./interfaces/IERC20.sol";
import { Interaction } from "./interfaces/ISettlement.sol";

/// @title A contract for impersonating a trader.
contract Trader {
    /// @dev Roundtrip a token in two CoW protocol settlements. First, buy some
    /// of the token being tested with the native token, then sell it back for
    /// the native token. This can be used as an indicator for token quality. If
    /// it is impossible to come up with roundtrip settlements, then the token
    /// is not supported.
    ///
    /// @param native - the native token (WETH on mainnet for example)
    /// @param token - the token to roundtrip and test for compatibility
    /// @param amountToken - the amount of token to buy and then sell
    /// @param native2token - the interactions for settling an trade buying the
    /// tested token with the native token
    /// @param token2native - the interactions for settling an trade selling the
    /// tested token for the native token
    function roundtrip(
        INativeERC20 native,
        IERC20 token,
        uint256 amountToken,
        Interaction[][3] calldata native2token,
        Interaction[][3] calldata token2native
    ) external returns (
        uint256 nativeSellAmount,
        uint256 nativeBuyAmount
    ) {
        revert("not implemented");
    }
}

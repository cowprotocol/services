// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IERC20 } from "./interfaces/IERC20.sol";
import { SafeERC20 } from "./libraries/SafeERC20.sol";

/// @title A piggy bank contract (Spardose is piggy bank in German)
/// @notice This contract account is used for pre-funding traders with tokens
/// for quote simulations. A separate contract is used (instead of overriding
/// the balance of the solver or trader directly) in order to interfere as
/// little as possible with the settlement.
contract Spardose {
    using SafeERC20 for *;

    /// @dev Request funds from the piggy bank to be transferred to the caller.
    /// Reverts if the transfer fails.
    ///
    /// @param token - the token request funds for
    /// @param amount - the amount of `token` to transfer
    function requestFunds(address token, uint256 amount) external {
        IERC20(token).safeTransfer(msg.sender, amount);
    }
}

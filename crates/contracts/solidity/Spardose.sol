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

    /// @dev Ensures that the trader has at least `amount` tokens. If not, it
    /// will transfer the difference to the trader.
    ///
    /// @param trader - the address of the trader
    /// @param token - the token to ensure a balance for
    /// @param amount - the amount of `token` that the `trader` must hold.
    function ensureBalance(address trader, address token, uint256 amount) external {
        uint256 traderBalance = IERC20(token).balanceOf(trader);
        if (traderBalance >= amount) {
            // Nothing to do.
            return;
        }

        uint256 difference = amount - traderBalance;
        require(
            IERC20(token).trySafeTransfer(trader, difference),
            "trader does not have enough sell_token"
        );
    }
}

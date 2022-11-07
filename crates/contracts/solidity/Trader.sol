// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IERC20, IPhonyERC20, INativeERC20 } from "./interfaces/IERC20.sol";
import { Interaction, Trade, SETTLEMENT } from "./interfaces/ISettlement.sol";
import { Caller } from "./libraries/Caller.sol";
import { Math } from "./libraries/Math.sol";
import { SafeERC20 } from "./libraries/SafeERC20.sol";

/// @title A contract for impersonating a trader.
contract Trader {
    using Caller for *;
    using Math for *;
    using SafeERC20 for *;

    /// @dev Simulates a executing a trade with the CoW protocol settlement
    /// contract. This sort of simulation provides stronger guarantees that the
    /// proposed trade is valid and would work in an actual settlement.
    ///
    /// @param tokens - tokens included in the settlement. Balances will be
    /// tracked for each token included in this array. `tokens[0]` is the trade
    /// sell token and `tokens[tokens.length - 1]` is the buy token.
    /// @param clearingPrices - the clearing prices for the settlement. This
    /// also doubles as the traded amounts, with `clearingPrices[0]` being the
    /// buy amount and the `clearingPrices[tokens.length - 1]` the sell amount.
    /// @param interactions - the interactions for settling the trade.
    /// @param mint - mint some sell token if this is a non-zero value. This
    /// requires that the sell token is mintable (which can be guaranteed by
    /// replacing its code with the `PhonyERC20` contract).
    ///
    /// @return gasUsed - the cumulative gas used for executing the simulated
    /// settlement.
    /// @return traderBalances - the changes in balances of the trader (`this`)
    /// for all tokens specified in the `tokens` array.
    /// @return settlementBalances - the changes in balances of the CoW protocol
    /// settlement contract for all tokens specified in the `tokens` array.
    function settle(
        address[] calldata tokens,
        uint256[] calldata clearingPrices,
        Interaction[][3] calldata interactions,
        uint256 mint
    ) external returns (
        uint256 gasUsed,
        int256[] memory traderBalances,
        int256[] memory settlementBalances
    ) {
        if (mint != 0) {
            IPhonyERC20(tokens[0]).mintPhonyTokens(address(this), mint);
        }
        // Make sure to reset the approval before setting a new one - some
        // popular tokens (like Tether USD) require this.
        IERC20(tokens[0]).safeApprove(address(SETTLEMENT.vaultRelayer()), 0);
        IERC20(tokens[0]).safeApprove(address(SETTLEMENT.vaultRelayer()), type(uint256).max);

        traderBalances = new int256[](tokens.length);
        settlementBalances = new int256[](tokens.length);
        for (uint256 i; i < tokens.length; ++i) {
            traderBalances[i] = -IERC20(tokens[i]).balanceOf(address(this)).toInt();
            settlementBalances[i] = -IERC20(tokens[i]).balanceOf(address(SETTLEMENT)).toInt();
        }

        Trade[] memory trades = new Trade[](1);
        trades[0] = Trade({
            sellTokenIndex: 0,
            buyTokenIndex: tokens.length - 1,
            receiver: address(0),
            sellAmount: clearingPrices[tokens.length - 1],
            buyAmount: clearingPrices[0],
            validTo: type(uint32).max,
            appData: bytes32(0),
            feeAmount: 0,
            flags: 0x40, // EIP-1271
            executedAmount: 0,
            signature: abi.encodePacked(address(this))
        });

        gasUsed = address(SETTLEMENT).doMeteredCallNoReturn(
            abi.encodeCall(
                SETTLEMENT.settle,
                (tokens, clearingPrices, trades, interactions)
            )
        );

        for (uint256 i; i < tokens.length; ++i) {
            traderBalances[i] += IERC20(tokens[i]).balanceOf(address(this)).toInt();
            settlementBalances[i] += IERC20(tokens[i]).balanceOf(address(SETTLEMENT)).toInt();
        }
    }

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

    /// @dev Validate all signature requests. This makes "signing" CoW protocol
    /// orders trivial.
    function isValidSignature(bytes32, bytes calldata) external pure returns (bytes4) {
        return 0x1626ba7e;
    }
}

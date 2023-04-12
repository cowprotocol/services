// SPDX-License-Identifier: LGPL-3.0-or-later
pragma solidity ^0.7.6;

import "../interfaces/IERC20.sol";
import "../mixins/GPv2Signing.sol";
import "./GPv2Order.sol";

/// @title Gnosis Protocol v2 Trade Library.
/// @author Gnosis Developers
library GPv2Trade {
    using GPv2Order for GPv2Order.Data;
    using GPv2Order for bytes;

    /// @dev A struct representing a trade to be executed as part a batch
    /// settlement.
    struct Data {
        uint256 sellTokenIndex;
        uint256 buyTokenIndex;
        address receiver;
        uint256 sellAmount;
        uint256 buyAmount;
        uint32 validTo;
        bytes32 appData;
        uint256 feeAmount;
        uint256 flags;
        uint256 executedAmount;
        bytes signature;
    }

    /// @dev Extracts the order data and signing scheme for the specified trade.
    ///
    /// @param trade The trade.
    /// @param tokens The list of tokens included in the settlement. The token
    /// indices in the trade parameters map to tokens in this array.
    /// @param order The memory location to extract the order data to.
    function extractOrder(
        Data calldata trade,
        IERC20[] calldata tokens,
        GPv2Order.Data memory order
    ) internal pure returns (GPv2Signing.Scheme signingScheme) {
        order.sellToken = tokens[trade.sellTokenIndex];
        order.buyToken = tokens[trade.buyTokenIndex];
        order.receiver = trade.receiver;
        order.sellAmount = trade.sellAmount;
        order.buyAmount = trade.buyAmount;
        order.validTo = trade.validTo;
        order.appData = trade.appData;
        order.feeAmount = trade.feeAmount;
        (
            order.kind,
            order.partiallyFillable,
            order.sellTokenBalance,
            order.buyTokenBalance,
            signingScheme
        ) = extractFlags(trade.flags);
    }

    /// @dev Decodes trade flags.
    ///
    /// Trade flags are used to tightly encode information on how to decode
    /// an order. Examples that directly affect the structure of an order are
    /// the kind of order (either a sell or a buy order) as well as whether the
    /// order is partially fillable or if it is a "fill-or-kill" order. It also
    /// encodes the signature scheme used to validate the order. As the most
    /// likely values are fill-or-kill sell orders by an externally owned
    /// account, the flags are chosen such that `0x00` represents this kind of
    /// order. The flags byte uses the following format:
    ///
    /// ```
    /// bit | 31 ...   | 6 | 5 | 4 | 3 | 2 | 1 | 0 |
    /// ----+----------+-------+---+-------+---+---+
    ///     | reserved | *   * | * | *   * | * | * |
    ///                  |   |   |   |   |   |   |
    ///                  |   |   |   |   |   |   +---- order kind bit, 0 for a sell order
    ///                  |   |   |   |   |   |         and 1 for a buy order
    ///                  |   |   |   |   |   |
    ///                  |   |   |   |   |   +-------- order fill bit, 0 for fill-or-kill
    ///                  |   |   |   |   |             and 1 for a partially fillable order
    ///                  |   |   |   |   |
    ///                  |   |   |   +---+------------ use internal sell token balance bit:
    ///                  |   |   |                     0x: ERC20 token balance
    ///                  |   |   |                     10: external Balancer Vault balance
    ///                  |   |   |                     11: internal Balancer Vault balance
    ///                  |   |   |
    ///                  |   |   +-------------------- use buy token balance bit
    ///                  |   |                         0: ERC20 token balance
    ///                  |   |                         1: internal Balancer Vault balance
    ///                  |   |
    ///                  +---+------------------------ signature scheme bits:
    ///                                                00: EIP-712
    ///                                                01: eth_sign
    ///                                                10: EIP-1271
    ///                                                11: pre_sign
    /// ```
    function extractFlags(uint256 flags)
        internal
        pure
        returns (
            bytes32 kind,
            bool partiallyFillable,
            bytes32 sellTokenBalance,
            bytes32 buyTokenBalance,
            GPv2Signing.Scheme signingScheme
        )
    {
        if (flags & 0x01 == 0) {
            kind = GPv2Order.KIND_SELL;
        } else {
            kind = GPv2Order.KIND_BUY;
        }
        partiallyFillable = flags & 0x02 != 0;
        if (flags & 0x08 == 0) {
            sellTokenBalance = GPv2Order.BALANCE_ERC20;
        } else if (flags & 0x04 == 0) {
            sellTokenBalance = GPv2Order.BALANCE_EXTERNAL;
        } else {
            sellTokenBalance = GPv2Order.BALANCE_INTERNAL;
        }
        if (flags & 0x10 == 0) {
            buyTokenBalance = GPv2Order.BALANCE_ERC20;
        } else {
            buyTokenBalance = GPv2Order.BALANCE_INTERNAL;
        }

        // NOTE: Take advantage of the fact that Solidity will revert if the
        // following expression does not produce a valid enum value. This means
        // we check here that the leading reserved bits must be 0.
        signingScheme = GPv2Signing.Scheme(flags >> 5);
    }
}

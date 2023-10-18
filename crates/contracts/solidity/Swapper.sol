// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IERC20 } from "./interfaces/IERC20.sol";
import { ISettlement, Interaction, Trade } from "./interfaces/ISettlement.sol";
import { Caller } from "./libraries/Caller.sol";
import { SafeERC20 } from "./libraries/SafeERC20.sol";

struct Asset {
    address token;
    uint256 amount;
}

struct Allowance {
    address spender;
    uint256 amount;
}

/// @title A contract for verifying DEX aggregator swaps for solving.
contract Swapper {
    using Caller for *;
    using SafeERC20 for *;

    /// @dev Simulates the execution of a single DEX swap over the CoW Protocol
    /// settlement contract. This is used for accurately simulating gas costs
    /// for orders with solver-computed fees.
    ///
    /// @param settlement - the CoW Protocol settlement contract.
    /// @param sell - the asset being sold in the swap.
    /// @param buy - the asset being bought in the swap.
    /// @param allowance - the required ERC-20 allowance for the swap; the
    /// approval will be me made on behalf of the settlement contract.
    /// @param call - the call for executing the swap.
    ///
    /// @return gasUsed - the cumulative gas used for executing the simulated
    /// settlement.
    function swap(
        ISettlement settlement,
        Asset calldata sell,
        Asset calldata buy,
        Allowance calldata allowance,
        Interaction calldata call
    ) external returns (
        uint256 gasUsed
    ) {
        if (IERC20(sell.token).balanceOf(address(this)) < sell.amount) {
            // The swapper does not have sufficient balance. This can happen
            // when hooks set up required balance for a trade. This is currently
            // not supported by this simulation, so return "0" to indicate that
            // no simulation was possible and that heuristic gas estimates
            // should be used instead.
            return 0;
        }

        // We first reset the allowance to 0 because some ERC20 tokens (e.g. USDT)
        // require that due to this attack:
        // https://github.com/ethereum/EIPs/issues/20#issuecomment-263524729
        // Before approving the amount we actually need.
        IERC20(sell.token).safeApprove(address(settlement.vaultRelayer()), 0);
        IERC20(sell.token).safeApprove(address(settlement.vaultRelayer()), sell.amount);

        address[] memory tokens = new address[](2);
        tokens[0] = sell.token;
        tokens[1] = buy.token;

        uint256[] memory clearingPrices = new uint256[](2);
        clearingPrices[0] = buy.amount;
        clearingPrices[1] = sell.amount;

        Trade[] memory trades = new Trade[](1);
        trades[0] = Trade({
            sellTokenIndex: 0,
            buyTokenIndex: 1,
            receiver: address(0),
            sellAmount: sell.amount,
            buyAmount: buy.amount,
            validTo: type(uint32).max,
            appData: bytes32(0),
            feeAmount: 0,
            flags: 0x40, // EIP-1271
            // Actual amount is irrelevant because we configure a fill-or-kill
            // order for which the settlement contract determines the
            // `executedAmount` automatically.
            executedAmount: 0,
            signature: abi.encodePacked(address(this))
        });

        Interaction[][3] memory interactions;
        if (
            IERC20(sell.token).allowance(address(settlement), allowance.spender)
                < allowance.amount
        ) {
            interactions[0] = new Interaction[](1);
            interactions[0][0].target = sell.token;
            interactions[0][0].callData = abi.encodeCall(
                IERC20(sell.token).approve,
                (allowance.spender, allowance.amount)
            );
        }
        interactions[1] = new Interaction[](1);
        interactions[1][0] = call;

        gasUsed = address(settlement).doMeteredCallNoReturn(
            abi.encodeCall(
                settlement.settle,
                (tokens, clearingPrices, trades, interactions)
            )
        );
    }

    /// @dev Validate all signature requests. This makes "signing" CoW protocol
    /// orders trivial.
    function isValidSignature(bytes32, bytes calldata) external pure returns (bytes4) {
        return 0x1626ba7e;
    }
}


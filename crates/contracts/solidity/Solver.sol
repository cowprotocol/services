// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IERC20, INativeERC20 } from "./interfaces/IERC20.sol";
import { Interaction, Trade, ISettlement } from "./interfaces/ISettlement.sol";
import { Caller } from "./libraries/Caller.sol";
import { Math } from "./libraries/Math.sol";
import { SafeERC20 } from "./libraries/SafeERC20.sol";
import { Spardose } from "./Spardose.sol";
import { Trader } from "./Trader.sol";

/// @title A contract for impersonating a solver. This contract
/// assumes that all solvers are EOAs so there is no fallback implementation
/// that proxies to some actual code. This way no solver can offer `ETH` from
/// their private liquidity for the solution which could interfere with gas
/// estimation.
contract Solver {
    using Caller for *;
    using Math for *;

    struct Mock {
        bool enabled;
        address spardose;
    }

    uint256 private _simulationOverhead;
    uint256[] private _queriedBalances;

    /// @dev Executes the given transaction from the context of a solver.
    /// That way we don't have to fake the authentication logic of the
    /// settlement contract as this address should actually be a
    /// verified solver.
    ///
    /// @param settlementContract - address of the settlement contract because
    /// it does not have a stable address in tests.
    /// @param trader - address of the order owner doing the trade
    /// @param sellToken - address of the token being sold
    /// @param sellAmount - amount being sold
    /// @param nativeToken - ERC20 version of the chain's token
    /// @param tokens - list of tokens used in the trade
    /// @param receiver - address receiving the bought tokens
    /// @param settlementCall - the calldata of the `settle()` call
    /// @param mock - mocking configuration for the simulation; this controls
    ///             whether things like ETH wrapping, setting allowance and
    ///             pre-funding should be done on behalf of the user to support
    ///             quote verification for users who aren't ready to swap.
    ///
    /// @return gasUsed - gas used for the `settle()` call
    /// @return queriedBalances - list of balances stored during the simulation
    function swap(
        ISettlement settlementContract,
        address payable trader,
        address sellToken,
        uint256 sellAmount,
        address nativeToken,
        address[] calldata tokens,
        address payable receiver,
        bytes calldata settlementCall,
        Mock memory mock
    ) external returns (
        uint256 gasUsed,
        uint256[] memory queriedBalances
    ) {
        require(msg.sender == address(this), "only simulation logic is allowed to call 'swap' function");

        if (mock.enabled) {
            // Prepare the trade in the context of the trader so we are allowed
            // to set approvals and things like that.
            Trader(trader)
                .prepareSwap(
                    settlementContract,
                    sellToken,
                    sellAmount,
                    nativeToken
                );

            // Ensure that the user has sufficient sell token balance for the
            // swap using balance overrides.
            Spardose(mock.spardose).ensureBalance(trader, sellToken, sellAmount);
        }

        // Warm the storage for sending ETH to smart contract addresses.
        // We allow this call to revert becaues it was either unnecessary in the first place
        // or failing to send `ETH` to the `receiver` will cause a revert in the settlement
        // contract.
        {
            (bool success,) = receiver.call{value: 0}("");
            success;
        }

        // Store pre-settlement balances
        _storeSettlementBalances(tokens, settlementContract);

        gasUsed = _executeSettlement(address(settlementContract), settlementCall);

        // Store post-settlement balances
        _storeSettlementBalances(tokens, settlementContract);

        queriedBalances = _queriedBalances;
    }

    /// @dev Helper function that reads the `owner`s balance for a given `token` and
    /// stores it. These stored balances will be returned as part of the simulation
    /// `Summary`.
    /// @param token - which token we read the balance from
    /// @param owner - whos balance we are reading
    /// @param countGas - controls whether this gas cost should be discounted from the settlement gas.
    function storeBalance(address token, address owner, bool countGas) external {
        uint256 gasStart = gasleft();
        _queriedBalances.push(
            token == 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE
                ? owner.balance
                : IERC20(token).balanceOf(owner)
        );
        if (countGas) {
            // Account for costs of gas used outside of metered section.
            _simulationOverhead += gasStart - gasleft() + 4460;
        }
    }

    /// @dev Helper function that reads and stores the balances of the `settlementContract` for each token in `tokens`.
    /// @param tokens - list of tokens used in the trade
    /// @param settlementContract - the settlement contract whose balances are being read
    function _storeSettlementBalances(address[] calldata tokens, ISettlement settlementContract) internal {
        for (uint256 i = 0; i < tokens.length; i++) {
            this.storeBalance(tokens[i], address(settlementContract), false);
        }
    }

    /// @dev Executes the settlement and measures the gas used.
    /// @param settlementContract The address of the settlement contract.
    /// @param settlementCall The calldata for the settlement function.
    /// @return gasUsed The amount of gas used during the settlement execution.
    function _executeSettlement(
        address settlementContract,
        bytes calldata settlementCall
    ) private returns (uint256 gasUsed) {
        uint256 gasStart = gasleft();
        address(settlementContract).doCall(settlementCall);
        gasUsed = gasStart - gasleft() - _simulationOverhead;
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import { IERC20, INativeERC20 } from "./interfaces/IERC20.sol";
import { Interaction, Trade, ISettlement } from "./interfaces/ISettlement.sol";
import { Caller } from "./libraries/Caller.sol";
import { Math } from "./libraries/Math.sol";
import { SafeERC20 } from "./libraries/SafeERC20.sol";
import { Spardose } from "./Spardose.sol";

/// @title A contract for impersonating a solver. This contract assume the solver
/// does not execute extra logic outside of the settlement that affects the execution
/// nor is called from the settlement. (TODO: remove this assumption by adding
/// a fallback handler delegating to the original solver account's code).
/// Because this contract code gets put at the address of a solver account it uses
/// a custom storage layout to avoid storage slot conflicts with solver accounts
/// that are smart contracts using the default layout.
/// layout at uint256(keccak256("cowprotocol/services solver impersonator"))
contract Solver layout at 0x14f5b2c185fc03c75c787d1f0e10ea137cc6d235a0047448eff18c9a173a722b {
    using Caller for *;
    using Math for *;

    uint256 private _simulationOverhead;
    uint256[] private _queriedBalances;

    /// @dev When setting up the simulation we compute state overrides to fund this
    /// address with the necessary sell tokens. If the user does not have the tokens
    /// already the solver will transfer them from this account to the user.
    address private constant PIGGY_BANK = 0x1111111111111111111111111111111111111111;

    /// @dev Executes the given transaction from the context of a solver.
    /// That way we don't have to fake the authentication logic of the
    /// settlement contract as this address should actually be a
    /// verified solver.
    ///
    /// @param settlementContract - address of the settlement contract because
    /// it does not have a stable address in tests.
    /// @param tokens - list of tokens used in the trade
    /// @param receiver - address receiving the bought tokens
    /// @param settlementCall - the calldata of the `settle()` call
    ///
    /// @return gasUsed - gas used for the `settle()` call
    /// @return queriedBalances - list of balances stored during the simulation
    function swap(
        ISettlement settlementContract,
        address[] calldata tokens,
        address payable receiver,
        bytes calldata settlementCall,
        address trader,
        address sellToken,
        uint256 sellAmount
    ) external returns (
        uint256 gasUsed,
        uint256[] memory queriedBalances
    ) {
        ensureTradePreconditions(settlementContract, trader, sellToken, sellAmount);
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

        gasUsed = _executeAndMeasure(settlementContract, settlementCall);

        // Store post-settlement balances
        _storeSettlementBalances(tokens, settlementContract);

        queriedBalances = _queriedBalances;
    }

    /// @dev Copies `settlementCall` to memory, invokes the settlement, and
    /// returns the net gas consumed (minus simulation overhead and the 2600
    /// cold-access cost that is normally covered by the 21K tx base cost).
    function _executeAndMeasure(
        ISettlement settlementContract,
        bytes calldata settlementCall
    ) internal returns (uint256 gasUsed) {
        // We copy the calldata to memory outside of the metered section because that
        // would normally not happen if the solver executes the settlement directly.
        bytes memory settlementCallMem = settlementCall;
        uint256 gasStart = gasleft();
        assembly {
            if iszero(call(gas(), settlementContract, 0, add(settlementCallMem, 32), mload(settlementCallMem), 0, 0)) {
                returndatacopy(0, 0, returndatasize())
                revert(0, returndatasize())
            }
        }
        gasUsed = gasStart - gasleft() - _simulationOverhead - 2600;
    }

    /// @dev Ensures that the user has given the necessary approvals and transfers sell
    /// tokens to the user if needed.
    /// @param settlementContract - pass in settlement contract because it does not have
    /// a stable address in tests.
    /// @param trader - account wanting to trade
    /// @param sellToken - token being sold by the trade
    /// @param sellAmount - expected amount to be sold according to the quote
    function ensureTradePreconditions(
        ISettlement settlementContract,
        address trader,
        address sellToken,
        uint256 sellAmount
    ) internal {
        address vaultRelayer = settlementContract.vaultRelayer();
        uint256 allowance = IERC20(sellToken).allowance(trader, vaultRelayer);

        // User did not actually give the required approval or we were not able
        // to compute the required state overrides.
        // Revert with a helpful message.
        require(allowance >= sellAmount, "trader did not give the required approvals");

        // Ensure that the user has sufficient sell token balance. If not, request some
        // funds from the piggy bank which will be available if balance overrides could
        // be computed correctly.
        uint256 sellBalance = IERC20(sellToken).balanceOf(trader);
        if (sellBalance < sellAmount) {
            try Spardose(PIGGY_BANK).requestFunds(trader, sellToken, sellAmount - sellBalance) {}
            catch {
                // The trader does not have sufficient sell token balance, and the
                // piggy bank pre-fund failed, as balance overrides are not available.
                // Revert with a helpful message.
                revert("trader does not have enough sell token");
            }
        }
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
}

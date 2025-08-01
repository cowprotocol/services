// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import { IERC20, INativeERC20 } from "./interfaces/IERC20.sol";
import { Interaction, Trade, ISettlement } from "./interfaces/ISettlement.sol";
import { Caller } from "./libraries/Caller.sol";
import { Math } from "./libraries/Math.sol";
import { SafeERC20 } from "./libraries/SafeERC20.sol";
import { Spardose } from "./Spardose.sol";

/// @title A contract for impersonating a trader.
/// Because this contract code gets put at the address of a trader account it uses
/// a custom storage layout to avoid storage slot conflicts with trader accounts
/// that are smart contracts using the default layout.
/// layout at uint256(keccak256("cowprotocol/services trader impersonator"))
contract Trader layout at 0x02565dba7d68dcbed629110024b7b5e785bfc1a484602045eea513de8a2dcf99 {
    using Caller for *;
    using Math for *;
    using SafeERC20 for *;

    bool private _initializationTriggered;

    /// @dev Address where the original code for the trader implementation is
    /// expected to be. Use 0x10000 as its the first "valid" address, since
    /// addresses up to 0xffff are reserved for pre-compiles.
    /// This is used to proxy calls to the original implementation in case
    /// the trader is actually a smart contract.
    address constant private TRADER_IMPL = address(0x10000);

    /// @dev Returns whether the trader initialization already happened and
    /// sets the flag to true.
    function triggerInitialization() private returns (bool value) {
        value = _initializationTriggered;
        _initializationTriggered = true;
    }

    // The `Trader` contract gets deployed on the `from` address of the quote.
    // Since the `from` address might be a safe or other smart contract we still
    // need to make the `Trader` behave as the original `from` would have in
    // case some custom interactions rely on that behavior.
    // To do that we simply implement fallback handlers that do delegate calls
    // to the original implementation.
    fallback() external payable {
        bytes memory rdata = TRADER_IMPL.doDelegatecall(msg.data);
        assembly { return(add(rdata, 32), mload(rdata)) }
    }
    // Proxying to the original trader implementation doesn't make sense since
    // smart contracts that do something on `receive()` are not supported by the
    // settlement contract anyway.
    receive() external payable {}

    /// @dev Executes needed actions on behalf of the trader to make the trade possible.
    ///      (e.g. wrapping ETH, setting approvals, and funding the account)
    /// @param settlementContract - pass in settlement contract because it does not have
    /// a stable address in tests.
    /// @param sellToken - token being sold by the trade
    /// @param sellAmount - expected amount to be sold according to the quote
    /// @param nativeToken - ERC20 version of the chain's native token
    /// @param spardose - piggy bank for requesting additional funds
    function ensureTradePreconditions(
        ISettlement settlementContract,
        address sellToken,
        uint256 sellAmount,
        address nativeToken,
        address spardose
    ) external {
        require(!triggerInitialization(), "prepareSwap can only be called once");

        if (sellToken == nativeToken) {
            uint256 availableNativeToken = IERC20(sellToken).balanceOf(address(this));
            if (availableNativeToken < sellAmount) {
                uint256 amountToWrap = sellAmount - availableNativeToken;
                // If the user has sufficient balance, simulate the wrapping the missing
                // `ETH` so the user doesn't have to spend gas on that just to get a quote.
                // If they are happy with the quote and want to create an order they will
                // actually have to do the wrapping, though. Note that we don't attempt to
                // wrap if the user doesn't have sufficient `ETH` balance, since that would
                // revert. Instead, we fall-through so that we handle insufficient sell
                // token balances uniformly for all tokens.
                if (address(this).balance >= amountToWrap) {
                    INativeERC20(nativeToken).deposit{value: amountToWrap}();
                }
            }
        }

        address vaultRelayer = settlementContract.vaultRelayer();
        uint256 currentAllowance = IERC20(sellToken).allowance(address(this), vaultRelayer);
        if (currentAllowance < sellAmount) {
            // Simulate an approval to the vault relayer so the user doesn't have to
            // spend gas on that just to get a quote. If they are happy with the quote and
            // want to create an order they will actually have to do the approvals, though.
            //
            // We first reset the allowance to 0 since some ERC20 tokens (e.g. USDT)
            // require that due to this attack:
            // https://github.com/ethereum/EIPs/issues/20#issuecomment-263524729
            //
            // In order to handle tokens which are not ERC20 compliant (like USDT) we have
            // to use `safeApprove()` instead of the regular `approve()` here.
            //
            // Some tokens revert when you try to set an approval to 0. To support these
            // tokens and USDT at the same time we catch any revert from the 2 approve calls.
            try this.safeApprove(sellToken, vaultRelayer, 0) {} catch {}
            try this.safeApprove(sellToken, vaultRelayer, type(uint256).max) {} catch {}
            uint256 allowance = IERC20(sellToken).allowance(address(this), vaultRelayer);
            require(allowance >= sellAmount, "trader did not give the required approvals");
        }

        // Ensure that the user has sufficient sell token balance. If not, request some
        // funds from the Spardose (piggy bank) which will be available if balance
        // overrides are enabled.
        uint256 sellBalance = IERC20(sellToken).balanceOf(address(this));
        if (sellBalance < sellAmount) {
            try Spardose(spardose).requestFunds(sellToken, sellAmount - sellBalance) {}
            catch {
                // The trader does not have sufficient sell token balance, and the
                // piggy bank pre-fund failed, as balance overrides are not available.
                // Revert with a helpful message.
                revert("trader does not have enough sell token");
            }
        }
    }

    /// @dev Wrap the `safeApprove` function in another function in order to mark it
    /// as `external`. That allows us to call it in a try-catch.
    function safeApprove(address token, address vaultRelayer, uint amount) external {
        IERC20(token).safeApprove(vaultRelayer, amount);
    }

    /// @dev Validate all signature requests. This makes "signing" CoW protocol
    /// orders trivial.
    function isValidSignature(bytes32, bytes calldata) external pure returns (bytes4) {
        return 0x1626ba7e;
    }
}

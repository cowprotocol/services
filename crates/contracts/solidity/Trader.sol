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

    /// @dev Address where the original code for the trader implementation is
    /// expected to be. Use 0x10000 as its the first "valid" address, since
    /// addresses up to 0xffff are reserved for pre-compiles.
    /// This is used to proxy calls to the original implementation in case
    /// the trader is actually a smart contract.
    address constant private TRADER_IMPL = address(0x10000);

    /// @dev Flag that ensures that `prepareSwap` gets called exactly once to
    /// prevent custom interaction from calling it.
    bool private _alreadyCalled = false;

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

    /// @dev Prepares everything needed by the trader for successfully executing the swap.
    /// This includes giving the required approval, wrapping the required ETH (if needed)
    /// and warming the needed storage for sending native ETH to smart contracts.
    /// @param sellToken - token being sold by the trade
    /// @param sellAmount - expected amount to be sold according to the quote
    /// @param nativeToken - ERC20 version of the chain's native token
    /// @param receiver - address that will receive the bought tokens
    function prepareSwap(
        address sellToken,
        uint256 sellAmount,
        address nativeToken,
        address payable receiver
    ) external {
        require(!_alreadyCalled, "prepareSwap can only be called once");
        _alreadyCalled = true;

        if (sellToken == nativeToken) {
            uint256 availableBalance = IERC20(sellToken).balanceOf(address(this));
            if (availableBalance < sellAmount) {
                // Simulate wrapping the missing `ETH` so the user doesn't have to spend gas
                // on that just to get a quote. If they are happy with the quote and want to
                // create an order they will actually have to do the wrapping, though.
                INativeERC20(nativeToken).deposit{value: sellAmount - availableBalance}();
            }
        }

        uint256 currentAllowance = IERC20(sellToken).allowance(address(this), address(SETTLEMENT.vaultRelayer()));
        if (currentAllowance < sellAmount) {
            // Simulate an approval to the settlement contract so the user doesn't have to
            // spend gas on that just to get a quote. If they are happy with the quote and
            // want to create an order they will actually have to do the approvals, though.
            // We first reset the allowance to 0 since some ERC20 tokens (e.g. USDT)
            // require that due to this attack:
            // https://github.com/ethereum/EIPs/issues/20#issuecomment-263524729
            IERC20(sellToken).safeApprove(address(SETTLEMENT.vaultRelayer()), 0);
            IERC20(sellToken).safeApprove(address(SETTLEMENT.vaultRelayer()), type(uint256).max);
        }

        // Warm the storage for sending ETH to smart contract addresses.
        // We allow this call to revert becaues it was either unnecessary in the first place
        // or failing to send `ETH` to the `receiver` will cause a revert in the settlement
        // contract.
        receiver.call{value: 0}("");
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

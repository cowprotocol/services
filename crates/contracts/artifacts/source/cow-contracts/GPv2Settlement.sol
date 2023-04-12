// SPDX-License-Identifier: LGPL-3.0-or-later
pragma solidity ^0.7.6;
pragma abicoder v2;

import "./GPv2VaultRelayer.sol";
import "./interfaces/GPv2Authentication.sol";
import "./interfaces/IERC20.sol";
import "./interfaces/IVault.sol";
import "./libraries/GPv2Interaction.sol";
import "./libraries/GPv2Order.sol";
import "./libraries/GPv2Trade.sol";
import "./libraries/GPv2Transfer.sol";
import "./libraries/SafeCast.sol";
import "./libraries/SafeMath.sol";
import "./mixins/GPv2Signing.sol";
import "./mixins/ReentrancyGuard.sol";
import "./mixins/StorageAccessible.sol";

/// @title Gnosis Protocol v2 Settlement Contract
/// @author Gnosis Developers
contract GPv2Settlement is GPv2Signing, ReentrancyGuard, StorageAccessible {
    using GPv2Order for bytes;
    using GPv2Transfer for IVault;
    using SafeCast for int256;
    using SafeCast for uint256;
    using SafeMath for uint256;

    /// @dev The authenticator is used to determine who can call the settle function.
    /// That is, only authorised solvers have the ability to invoke settlements.
    /// Any valid authenticator implements an isSolver method called by the onlySolver
    /// modifier below.
    GPv2Authentication public immutable authenticator;

    /// @dev The Balancer Vault the protocol uses for managing user funds.
    IVault public immutable vault;

    /// @dev The Balancer Vault relayer which can interact on behalf of users.
    /// This contract is created during deployment
    GPv2VaultRelayer public immutable vaultRelayer;

    /// @dev Map each user order by UID to the amount that has been filled so
    /// far. If this amount is larger than or equal to the amount traded in the
    /// order (amount sold for sell orders, amount bought for buy orders) then
    /// the order cannot be traded anymore. If the order is fill or kill, then
    /// this value is only used to determine whether the order has already been
    /// executed.
    mapping(bytes => uint256) public filledAmount;

    /// @dev Event emitted for each executed trade.
    event Trade(
        address indexed owner,
        IERC20 sellToken,
        IERC20 buyToken,
        uint256 sellAmount,
        uint256 buyAmount,
        uint256 feeAmount,
        bytes orderUid
    );

    /// @dev Event emitted for each executed interaction.
    ///
    /// For gas effeciency, only the interaction calldata selector (first 4
    /// bytes) is included in the event. For interactions without calldata or
    /// whose calldata is shorter than 4 bytes, the selector will be `0`.
    event Interaction(address indexed target, uint256 value, bytes4 selector);

    /// @dev Event emitted when a settlement complets
    event Settlement(address indexed solver);

    /// @dev Event emitted when an order is invalidated.
    event OrderInvalidated(address indexed owner, bytes orderUid);

    constructor(GPv2Authentication authenticator_, IVault vault_) {
        authenticator = authenticator_;
        vault = vault_;
        vaultRelayer = new GPv2VaultRelayer(vault_);
    }

    // solhint-disable-next-line no-empty-blocks
    receive() external payable {
        // NOTE: Include an empty receive function so that the settlement
        // contract can receive Ether from contract interactions.
    }

    /// @dev This modifier is called by settle function to block any non-listed
    /// senders from settling batches.
    modifier onlySolver() {
        require(authenticator.isSolver(msg.sender), "GPv2: not a solver");
        _;
    }

    /// @dev Modifier to ensure that an external function is only callable as a
    /// settlement interaction.
    modifier onlyInteraction() {
        require(address(this) == msg.sender, "GPv2: not an interaction");
        _;
    }

    /// @dev Settle the specified orders at a clearing price. Note that it is
    /// the responsibility of the caller to ensure that all GPv2 invariants are
    /// upheld for the input settlement, otherwise this call will revert.
    /// Namely:
    /// - All orders are valid and signed
    /// - Accounts have sufficient balance and approval.
    /// - Settlement contract has sufficient balance to execute trades. Note
    ///   this implies that the accumulated fees held in the contract can also
    ///   be used for settlement. This is OK since:
    ///   - Solvers need to be authorized
    ///   - Misbehaving solvers will be slashed for abusing accumulated fees for
    ///     settlement
    ///   - Critically, user orders are entirely protected
    ///
    /// @param tokens An array of ERC20 tokens to be traded in the settlement.
    /// Trades encode tokens as indices into this array.
    /// @param clearingPrices An array of clearing prices where the `i`-th price
    /// is for the `i`-th token in the [`tokens`] array.
    /// @param trades Trades for signed orders.
    /// @param interactions Smart contract interactions split into three
    /// separate lists to be run before the settlement, during the settlement
    /// and after the settlement respectively.
    function settle(
        IERC20[] calldata tokens,
        uint256[] calldata clearingPrices,
        GPv2Trade.Data[] calldata trades,
        GPv2Interaction.Data[][3] calldata interactions
    ) external nonReentrant onlySolver {
        executeInteractions(interactions[0]);

        (
            GPv2Transfer.Data[] memory inTransfers,
            GPv2Transfer.Data[] memory outTransfers
        ) = computeTradeExecutions(tokens, clearingPrices, trades);

        vaultRelayer.transferFromAccounts(inTransfers);

        executeInteractions(interactions[1]);

        vault.transferToAccounts(outTransfers);

        executeInteractions(interactions[2]);

        emit Settlement(msg.sender);
    }

    /// @dev Settle an order directly against Balancer V2 pools.
    ///
    /// @param swaps The Balancer V2 swap steps to use for trading.
    /// @param tokens An array of ERC20 tokens to be traded in the settlement.
    /// Swaps and the trade encode tokens as indices into this array.
    /// @param trade The trade to match directly against Balancer liquidity. The
    /// order will always be fully executed, so the trade's `executedAmount`
    /// field is used to represent a swap limit amount.
    function swap(
        IVault.BatchSwapStep[] calldata swaps,
        IERC20[] calldata tokens,
        GPv2Trade.Data calldata trade
    ) external nonReentrant onlySolver {
        RecoveredOrder memory recoveredOrder = allocateRecoveredOrder();
        GPv2Order.Data memory order = recoveredOrder.data;
        recoverOrderFromTrade(recoveredOrder, tokens, trade);

        IVault.SwapKind kind = order.kind == GPv2Order.KIND_SELL
            ? IVault.SwapKind.GIVEN_IN
            : IVault.SwapKind.GIVEN_OUT;

        IVault.FundManagement memory funds;
        funds.sender = recoveredOrder.owner;
        funds.fromInternalBalance =
            order.sellTokenBalance == GPv2Order.BALANCE_INTERNAL;
        funds.recipient = payable(recoveredOrder.receiver);
        funds.toInternalBalance =
            order.buyTokenBalance == GPv2Order.BALANCE_INTERNAL;

        int256[] memory limits = new int256[](tokens.length);
        uint256 limitAmount = trade.executedAmount;
        // NOTE: Array allocation initializes elements to 0, so we only need to
        // set the limits we care about. This ensures that the swap will respect
        // the order's limit price.
        if (order.kind == GPv2Order.KIND_SELL) {
            require(limitAmount >= order.buyAmount, "GPv2: limit too low");
            limits[trade.sellTokenIndex] = order.sellAmount.toInt256();
            limits[trade.buyTokenIndex] = -limitAmount.toInt256();
        } else {
            require(limitAmount <= order.sellAmount, "GPv2: limit too high");
            limits[trade.sellTokenIndex] = limitAmount.toInt256();
            limits[trade.buyTokenIndex] = -order.buyAmount.toInt256();
        }

        GPv2Transfer.Data memory feeTransfer;
        feeTransfer.account = recoveredOrder.owner;
        feeTransfer.token = order.sellToken;
        feeTransfer.amount = order.feeAmount;
        feeTransfer.balance = order.sellTokenBalance;

        int256[] memory tokenDeltas = vaultRelayer.batchSwapWithFee(
            kind,
            swaps,
            tokens,
            funds,
            limits,
            // NOTE: Specify a deadline to ensure that an expire order
            // cannot be used to trade.
            order.validTo,
            feeTransfer
        );

        bytes memory orderUid = recoveredOrder.uid;
        uint256 executedSellAmount = tokenDeltas[trade.sellTokenIndex]
            .toUint256();
        uint256 executedBuyAmount = (-tokenDeltas[trade.buyTokenIndex])
            .toUint256();

        // NOTE: Check that the orders were completely filled and update their
        // filled amounts to avoid replaying them. The limit price and order
        // validity have already been verified when executing the swap through
        // the `limit` and `deadline` parameters.
        require(filledAmount[orderUid] == 0, "GPv2: order filled");
        if (order.kind == GPv2Order.KIND_SELL) {
            require(
                executedSellAmount == order.sellAmount,
                "GPv2: sell amount not respected"
            );
            filledAmount[orderUid] = order.sellAmount;
        } else {
            require(
                executedBuyAmount == order.buyAmount,
                "GPv2: buy amount not respected"
            );
            filledAmount[orderUid] = order.buyAmount;
        }

        emit Trade(
            recoveredOrder.owner,
            order.sellToken,
            order.buyToken,
            executedSellAmount,
            executedBuyAmount,
            order.feeAmount,
            orderUid
        );
        emit Settlement(msg.sender);
    }

    /// @dev Invalidate onchain an order that has been signed offline.
    ///
    /// @param orderUid The unique identifier of the order that is to be made
    /// invalid after calling this function. The user that created the order
    /// must be the the sender of this message. See [`extractOrderUidParams`]
    /// for details on orderUid.
    function invalidateOrder(bytes calldata orderUid) external {
        (, address owner, ) = orderUid.extractOrderUidParams();
        require(owner == msg.sender, "GPv2: caller does not own order");
        filledAmount[orderUid] = uint256(-1);
        emit OrderInvalidated(owner, orderUid);
    }

    /// @dev Free storage from the filled amounts of **expired** orders to claim
    /// a gas refund. This method can only be called as an interaction.
    ///
    /// @param orderUids The unique identifiers of the expired order to free
    /// storage for.
    function freeFilledAmountStorage(bytes[] calldata orderUids)
        external
        onlyInteraction
    {
        freeOrderStorage(filledAmount, orderUids);
    }

    /// @dev Free storage from the pre signatures of **expired** orders to claim
    /// a gas refund. This method can only be called as an interaction.
    ///
    /// @param orderUids The unique identifiers of the expired order to free
    /// storage for.
    function freePreSignatureStorage(bytes[] calldata orderUids)
        external
        onlyInteraction
    {
        freeOrderStorage(preSignature, orderUids);
    }

    /// @dev Process all trades one at a time returning the computed net in and
    /// out transfers for the trades.
    ///
    /// This method reverts if processing of any single trade fails. See
    /// [`computeTradeExecution`] for more details.
    ///
    /// @param tokens An array of ERC20 tokens to be traded in the settlement.
    /// @param clearingPrices An array of token clearing prices.
    /// @param trades Trades for signed orders.
    /// @return inTransfers Array of in transfers of executed sell amounts.
    /// @return outTransfers Array of out transfers of executed buy amounts.
    function computeTradeExecutions(
        IERC20[] calldata tokens,
        uint256[] calldata clearingPrices,
        GPv2Trade.Data[] calldata trades
    )
        internal
        returns (
            GPv2Transfer.Data[] memory inTransfers,
            GPv2Transfer.Data[] memory outTransfers
        )
    {
        RecoveredOrder memory recoveredOrder = allocateRecoveredOrder();

        inTransfers = new GPv2Transfer.Data[](trades.length);
        outTransfers = new GPv2Transfer.Data[](trades.length);

        for (uint256 i = 0; i < trades.length; i++) {
            GPv2Trade.Data calldata trade = trades[i];

            recoverOrderFromTrade(recoveredOrder, tokens, trade);
            computeTradeExecution(
                recoveredOrder,
                clearingPrices[trade.sellTokenIndex],
                clearingPrices[trade.buyTokenIndex],
                trade.executedAmount,
                inTransfers[i],
                outTransfers[i]
            );
        }
    }

    /// @dev Compute the in and out transfer amounts for a single trade.
    /// This function reverts if:
    /// - The order has expired
    /// - The order's limit price is not respected
    /// - The order gets over-filled
    /// - The fee discount is larger than the executed fee
    ///
    /// @param recoveredOrder The recovered order to process.
    /// @param sellPrice The price of the order's sell token.
    /// @param buyPrice The price of the order's buy token.
    /// @param executedAmount The portion of the order to execute. This will be
    /// ignored for fill-or-kill orders.
    /// @param inTransfer Memory location for computed executed sell amount
    /// transfer.
    /// @param outTransfer Memory location for computed executed buy amount
    /// transfer.
    function computeTradeExecution(
        RecoveredOrder memory recoveredOrder,
        uint256 sellPrice,
        uint256 buyPrice,
        uint256 executedAmount,
        GPv2Transfer.Data memory inTransfer,
        GPv2Transfer.Data memory outTransfer
    ) internal {
        GPv2Order.Data memory order = recoveredOrder.data;
        bytes memory orderUid = recoveredOrder.uid;

        // solhint-disable-next-line not-rely-on-time
        require(order.validTo >= block.timestamp, "GPv2: order expired");

        // NOTE: The following computation is derived from the equation:
        // ```
        // amount_x * price_x = amount_y * price_y
        // ```
        // Intuitively, if a chocolate bar is 0,50€ and a beer is 4€, 1 beer
        // is roughly worth 8 chocolate bars (`1 * 4 = 8 * 0.5`). From this
        // equation, we can derive:
        // - The limit price for selling `x` and buying `y` is respected iff
        // ```
        // limit_x * price_x >= limit_y * price_y
        // ```
        // - The executed amount of token `y` given some amount of `x` and
        //   clearing prices is:
        // ```
        // amount_y = amount_x * price_x / price_y
        // ```

        require(
            order.sellAmount.mul(sellPrice) >= order.buyAmount.mul(buyPrice),
            "GPv2: limit price not respected"
        );

        uint256 executedSellAmount;
        uint256 executedBuyAmount;
        uint256 executedFeeAmount;
        uint256 currentFilledAmount;

        if (order.kind == GPv2Order.KIND_SELL) {
            if (order.partiallyFillable) {
                executedSellAmount = executedAmount;
                executedFeeAmount = order.feeAmount.mul(executedSellAmount).div(
                        order.sellAmount
                    );
            } else {
                executedSellAmount = order.sellAmount;
                executedFeeAmount = order.feeAmount;
            }

            executedBuyAmount = executedSellAmount.mul(sellPrice).ceilDiv(
                buyPrice
            );

            currentFilledAmount = filledAmount[orderUid].add(
                executedSellAmount
            );
            require(
                currentFilledAmount <= order.sellAmount,
                "GPv2: order filled"
            );
        } else {
            if (order.partiallyFillable) {
                executedBuyAmount = executedAmount;
                executedFeeAmount = order.feeAmount.mul(executedBuyAmount).div(
                    order.buyAmount
                );
            } else {
                executedBuyAmount = order.buyAmount;
                executedFeeAmount = order.feeAmount;
            }

            executedSellAmount = executedBuyAmount.mul(buyPrice).div(sellPrice);

            currentFilledAmount = filledAmount[orderUid].add(executedBuyAmount);
            require(
                currentFilledAmount <= order.buyAmount,
                "GPv2: order filled"
            );
        }

        executedSellAmount = executedSellAmount.add(executedFeeAmount);
        filledAmount[orderUid] = currentFilledAmount;

        emit Trade(
            recoveredOrder.owner,
            order.sellToken,
            order.buyToken,
            executedSellAmount,
            executedBuyAmount,
            executedFeeAmount,
            orderUid
        );

        inTransfer.account = recoveredOrder.owner;
        inTransfer.token = order.sellToken;
        inTransfer.amount = executedSellAmount;
        inTransfer.balance = order.sellTokenBalance;

        outTransfer.account = recoveredOrder.receiver;
        outTransfer.token = order.buyToken;
        outTransfer.amount = executedBuyAmount;
        outTransfer.balance = order.buyTokenBalance;
    }

    /// @dev Execute a list of arbitrary contract calls from this contract.
    /// @param interactions The list of interactions to execute.
    function executeInteractions(GPv2Interaction.Data[] calldata interactions)
        internal
    {
        for (uint256 i; i < interactions.length; i++) {
            GPv2Interaction.Data calldata interaction = interactions[i];

            // To prevent possible attack on user funds, we explicitly disable
            // any interactions with the vault relayer contract.
            require(
                interaction.target != address(vaultRelayer),
                "GPv2: forbidden interaction"
            );
            GPv2Interaction.execute(interaction);

            emit Interaction(
                interaction.target,
                interaction.value,
                GPv2Interaction.selector(interaction)
            );
        }
    }

    /// @dev Claims refund for the specified storage and order UIDs.
    ///
    /// This method reverts if any of the orders are still valid.
    ///
    /// @param orderUids Order refund data for freeing storage.
    /// @param orderStorage Order storage mapped on a UID.
    function freeOrderStorage(
        mapping(bytes => uint256) storage orderStorage,
        bytes[] calldata orderUids
    ) internal {
        for (uint256 i = 0; i < orderUids.length; i++) {
            bytes calldata orderUid = orderUids[i];

            (, , uint32 validTo) = orderUid.extractOrderUidParams();
            // solhint-disable-next-line not-rely-on-time
            require(validTo < block.timestamp, "GPv2: order still valid");

            orderStorage[orderUid] = 0;
        }
    }
}

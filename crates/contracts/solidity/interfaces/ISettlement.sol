// SPDX-License-Identifier: MIT
pragma solidity ^0.8.16;

ISettlement constant SETTLEMENT = ISettlement(0x9008D19f58AAbD9eD0D60971565AA8510560ab41);

struct Interaction {
    address target;
    uint256 value;
    bytes callData;
}

struct Trade {
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

/// @title CoW protocol settlement contract interface
interface ISettlement {
    function domainSeparator() external view returns (bytes32);
    function authenticator() external view returns (address);
    function vaultRelayer() external view returns (address);
    function setPreSignature(bytes calldata orderUid, bool signed) external;
    function settle(
        address[] calldata tokens,
        uint256[] calldata clearingPrices,
        Trade[] calldata trades,
        Interaction[][3] calldata interactions
    ) external;
}

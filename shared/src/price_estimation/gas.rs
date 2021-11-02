///! constants to estimate gas use in GPv2

/// minimum gas every settlement takes
pub const SETTLEMENT: u64 =
    // initial tx gas
    32_000 +
    // isSolver
    7365;

/// gas per trade excluding c20 transfer
pub const TRADE: u64 =
    // computeTradeExecutions
    35_000 +
    // transferFromAccounts and transferToAccount overhead
    2 * 3000 +
    // overhead of one interaction
    3000;

/// lower bound for an erc20 transfer.
///
/// Value was computed by taking 52 percentile median of `transfer()` costs
/// of the 90% most traded tokens by volume in the month of Oct. 2021.
pub const ERC20_TRANSFER: u64 = 27_513;

/// a settlement that contains one trade
pub const SETTLEMENT_SINGLE_TRADE: u64 = SETTLEMENT + TRADE + 2 * ERC20_TRANSFER;

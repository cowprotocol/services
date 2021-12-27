///! constants to estimate gas use in GPv2

/// gas for initialization
pub const INITIALIZATION_COST: u64 =
    // initial tx gas
    32_000;

/// minimum gas every settlement takes
pub const SETTLEMENT: u64 =
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

/// lower bound for gas refunds
/// this number was derived from some empiric observations
pub const GAS_REFUNDS: u64 = 13_000;

/// a settlement that contains one trade
pub const SETTLEMENT_SINGLE_TRADE: u64 =
    INITIALIZATION_COST + SETTLEMENT + TRADE + 2 * ERC20_TRANSFER - GAS_REFUNDS;

/// settlement overhead for one trade
pub const SETTLEMENT_OVERHEAD: u64 = SETTLEMENT + TRADE + 2 * ERC20_TRANSFER;

/// lower bound for execution of one order
///
/// Estimates from multivariate linear regression here:
/// https://docs.google.com/spreadsheets/d/13UeUQ9DA4bHlcy9-i8d4nSLlCxSfjcXpTelvXYzyJzQ/edit?usp=sharing
pub static GAS_PER_ORDER: u64 = 66_315;

/// lower bound for executing one trade on uniswap
pub static GAS_PER_UNISWAP: u64 = 94_696;

/// lower bound for executing one trade on balancer
///
/// Taken from a sample of two swaps
/// https://etherscan.io/tx/0x72d234d35fd169ef497ba0a1dc23258c96f278fb688d375d135eb012e5311009
/// https://etherscan.io/tx/0x1c345a6da1edb2bba953685a4cf85f6a0d967ac751f8c5b518578c5fd20a7c96
pub static GAS_PER_BALANCER_SWAP: u64 = 120_000;

pub static GAS_PER_WETH_UNWRAP: u64 = 14_192;

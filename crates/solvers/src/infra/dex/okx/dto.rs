//! DTOs for the OKX swap API. Full documentation for the API can be found
//! [here](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap).

use {
    crate::{
        domain::{
            dex,
            eth::{self, TokenAddress},
            order,
        },
        util::serialize,
    },
    alloy::primitives::U256,
    bigdecimal::BigDecimal,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// A OKX API swap request parameters (only mandatory fields).
/// OKX v6 supports both sell orders (exactIn) and buy orders (exactOut).
///
/// See [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    /// Chain ID
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_index: u64,

    /// Input amount of a token to be sold or bought set in minimal divisible
    /// units.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,

    /// Contract address of a token to be sent
    pub from_token_address: eth::Address,

    /// Contract address of a token to be received
    pub to_token_address: eth::Address,

    /// Limit of price slippage you are willing to accept
    pub slippage_percent: Slippage,

    /// User's wallet address. Where the sell tokens will be taken from.
    pub user_wallet_address: eth::Address,

    /// Where the buy tokens get sent to.
    pub swap_receiver_address: eth::Address,

    /// Swap mode: "exactIn" for sell orders (default), "exactOut" for buy
    /// orders
    pub swap_mode: SwapMode,

    /// The percentage of the price impact allowed.
    /// When set to 100%, the feature is disabled.
    /// OKX API default is 90% if this parameter is not sent.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub price_impact_protection_percent: BigDecimal,
}

/// A OKX slippage amount.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Slippage(BigDecimal);

/// A OKX swap mode.
#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SwapMode {
    #[default]
    ExactIn,
    ExactOut,
}

impl SwapRequest {
    pub fn with_domain(self, order: &dex::Order, slippage: &dex::Slippage) -> Self {
        let swap_mode = match order.side {
            order::Side::Sell => SwapMode::ExactIn,
            order::Side::Buy => SwapMode::ExactOut,
        };

        Self {
            from_token_address: order.sell.0,
            to_token_address: order.buy.0,
            amount: order.amount.get(),
            slippage_percent: Slippage(slippage.as_factor().clone()),
            swap_mode,
            ..self
        }
    }
}

/// A OKX API V5 swap request parameters (only mandatory fields).
/// Currently, only V5 API supports buy orders.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequestV5 {
    /// Chain ID (V5 uses chainId instead of chainIndex)
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_id: u64,

    /// Input amount of a token to be sold or bought set in minimal divisible
    /// units.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,

    /// Contract address of a token to be sent
    pub from_token_address: eth::Address,

    /// Contract address of a token to be received
    pub to_token_address: eth::Address,

    /// Limit of price slippage you are willing to accept (V5 uses slippage
    /// instead of slippagePercent)
    pub slippage: Slippage,

    /// User's wallet address. Where the sell tokens will be taken from.
    pub user_wallet_address: eth::Address,

    /// Where the buy tokens get sent to.
    pub swap_receiver_address: eth::Address,

    /// Swap mode: "exactIn" for sell orders (default), "exactOut" for buy
    /// orders
    pub swap_mode: SwapMode,

    /// The percentage of the price impact allowed.
    /// When set to 100%, the feature is disabled.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub price_impact_protection_percent: BigDecimal,
}

impl From<&SwapRequest> for SwapRequestV5 {
    fn from(v6_request: &SwapRequest) -> Self {
        Self {
            chain_id: v6_request.chain_index,
            amount: v6_request.amount,
            from_token_address: v6_request.from_token_address,
            to_token_address: v6_request.to_token_address,
            slippage: v6_request.slippage_percent.clone(),
            user_wallet_address: v6_request.user_wallet_address,
            swap_receiver_address: v6_request.swap_receiver_address,
            swap_mode: v6_request.swap_mode.clone(),
            price_impact_protection_percent: v6_request.price_impact_protection_percent.clone(),
        }
    }
}

/// A OKX API swap response.
///
/// See [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    /// Quote execution path.
    pub router_result: SwapResponseRouterResult,

    /// Contract related response.
    pub tx: SwapResponseTx,
}

/// A OKX API swap response - quote execution path.
/// Deserializing fields which are only used by the implementation.
/// For all possible fields look into the documentation:
/// [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseRouterResult {
    /// The information of a token to be sold.
    pub from_token: SwapResponseFromToToken,

    /// The information of a token to be bought.
    pub to_token: SwapResponseFromToToken,

    /// The input amount of a token to be sold.
    #[serde_as(as = "serialize::U256")]
    pub from_token_amount: U256,

    /// The resulting amount of a token to be bought.
    #[serde_as(as = "serialize::U256")]
    pub to_token_amount: U256,
}

/// A OKX API swap response - token information.
/// Deserializing fields which are only used by the implementation.
/// For all possible fields look into the documentation:
/// [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseFromToToken {
    /// Address of the token smart contract.
    pub token_contract_address: eth::Address,
}

/// A OKX API swap response - contract related information.
/// Deserializing fields which are only used by the implementation.
/// For all possible fields look into the documentation:
/// [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseTx {
    /// Estimated amount of the gas limit.
    #[serde_as(as = "serialize::U256")]
    pub gas: U256,

    /// The contract address of OKX DEX router.
    pub to: eth::Address,

    /// Call data.
    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,
}

/// A OKX API approve transaction request.
///
/// See [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-approve-transaction)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveTransactionRequest {
    /// Chain ID
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_index: u64,

    /// Contract address of a token to be permitted.
    pub token_contract_address: eth::Address,

    /// The amount of token that needs to be permitted (in minimal divisible
    /// units).
    #[serde_as(as = "serialize::U256")]
    pub approve_amount: U256,
}

impl ApproveTransactionRequest {
    pub fn new(chain_index: u64, token: TokenAddress, amount: U256) -> Self {
        Self {
            chain_index,
            token_contract_address: token.0,
            approve_amount: amount,
        }
    }
}

/// A OKX API V5 approve transaction request.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveTransactionRequestV5 {
    /// Chain ID (V5 uses chainId instead of chainIndex)
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_id: u64,

    /// Contract address of a token to be permitted.
    pub token_contract_address: eth::Address,

    /// The amount of token that needs to be permitted (in minimal divisible
    /// units).
    #[serde_as(as = "serialize::U256")]
    pub approve_amount: U256,
}

impl From<&ApproveTransactionRequest> for ApproveTransactionRequestV5 {
    fn from(v6_request: &ApproveTransactionRequest) -> Self {
        Self {
            chain_id: v6_request.chain_index,
            token_contract_address: v6_request.token_contract_address,
            approve_amount: v6_request.approve_amount,
        }
    }
}

/// A OKX API approve transaction response.
/// Deserializing fields which are only used by the implementation.
/// See [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-approve-transaction)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveTransactionResponse {
    /// The contract address of OKX DEX approve.
    pub dex_contract_address: eth::Address,
}

/// A OKX API response - generic wrapper for success and failure cases.
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Response<T> {
    /// Error code, 0 for success, otherwise one of:
    /// [error codes](https://web3.okx.com/build/dev-docs/wallet-api/dex-error-code)
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub code: i64,

    /// Response data.
    pub data: Vec<T>,

    /// Error code text message.
    pub msg: String,
}

#[derive(Deserialize)]
pub struct Error {
    pub code: i64,
    pub reason: String,
}

//! DTOs for the Bitget swap API.
//! Full documentation: https://web3.bitget.com/en/docs/swap/

use {
    crate::domain::{dex, eth},
    bigdecimal::{BigDecimal, ToPrimitive},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// Bitget chain name used in API requests.
#[derive(Clone, Copy, Serialize)]
pub enum ChainName {
    #[serde(rename = "eth")]
    Mainnet,
    #[serde(rename = "bsc")]
    Bnb,
    #[serde(rename = "base")]
    Base,
    #[serde(rename = "polygon")]
    Polygon,
    #[serde(rename = "arb")]
    ArbitrumOne,
}

impl ChainName {
    pub fn new(chain_id: eth::ChainId) -> Self {
        match chain_id {
            eth::ChainId::Mainnet => Self::Mainnet,
            eth::ChainId::Bnb => Self::Bnb,
            eth::ChainId::Base => Self::Base,
            eth::ChainId::Polygon => Self::Polygon,
            eth::ChainId::ArbitrumOne => Self::ArbitrumOne,
            _ => panic!("unsupported Bitget chain: {chain_id:?}"),
        }
    }
}

/// A Bitget API swap request with enriched response (`requestMod = "rich"`).
///
/// See [API](https://web3.bitget.com/en/docs/swap/)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    /// Source token contract address.
    pub from_contract: eth::Address,

    /// Input amount in human-readable decimal units (e.g. "1" for 1 WETH).
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub from_amount: BigDecimal,

    /// Source chain name.
    pub from_chain: ChainName,

    /// Target token contract address.
    pub to_contract: eth::Address,

    /// Target chain name.
    pub to_chain: ChainName,

    /// Debit address.
    pub from_address: eth::Address,

    /// Recipient address.
    pub to_address: eth::Address,

    /// Optimal channel - hardcoded to "bgwevmaggregator" for EVM chains.
    pub market: String,

    /// Slippage tolerance as a percentage (e.g. 1.0 for 1%).
    pub slippage: f64,

    /// Request mode - "rich" returns quote data alongside swap calldata.
    pub request_mod: String,

    /// Fee rate in per mille. 0 for no fee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_rate: Option<f64>,
}

impl SwapRequest {
    pub fn from_order(
        order: &dex::Order,
        chain_name: ChainName,
        settlement_contract: eth::Address,
        slippage: &dex::Slippage,
        sell_decimals: u8,
    ) -> Self {
        Self {
            from_contract: order.sell.0,
            from_amount: super::wei_to_decimal(order.amount.get(), sell_decimals),
            from_chain: chain_name,
            to_contract: order.buy.0,
            to_chain: chain_name,
            from_address: settlement_contract,
            to_address: settlement_contract,
            market: "bgwevmaggregator".to_string(),
            slippage: slippage.as_factor().to_f64().unwrap_or_default() * 100.0,
            request_mod: "rich".to_string(),
            fee_rate: Some(0.0),
        }
    }
}

/// A Bitget API enriched swap response (returned when `requestMod = "rich"`).
#[serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    /// Output amount in decimal units.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub out_amount: BigDecimal,

    /// Gas fee information.
    pub gas_fee: GasFee,

    /// Transaction data for the swap.
    pub swap_transaction: SwapTransaction,
}

#[serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GasFee {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub gas_limit: u64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SwapTransaction {
    /// Contract address (router/spender).
    pub to: eth::Address,
    /// Hex-encoded calldata with "0x" prefix.
    pub data: String,
}

impl SwapTransaction {
    /// Decode the hex-encoded calldata (with "0x" prefix) to bytes.
    pub fn decode_calldata(&self) -> Result<Vec<u8>, hex::FromHexError> {
        let hex_str = self.data.strip_prefix("0x").unwrap_or(&self.data);
        hex::decode(hex_str)
    }
}

/// A Bitget API response wrapper.
///
/// On success `status` is 0 and `data` contains the result.
/// On error `status` is non-zero and `data` is null.
#[derive(Deserialize, Clone, Debug)]
pub struct Response<T> {
    /// Response status code (0 = success).
    pub status: i64,

    /// Response data — `None` when the API returns an error.
    pub data: Option<T>,
}

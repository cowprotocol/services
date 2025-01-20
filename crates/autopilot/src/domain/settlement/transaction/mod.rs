use {
    crate::{
        boundary,
        domain::{self, auction::order, eth},
    },
    ethcontract::common::FunctionExt,
    std::sync::LazyLock,
};

mod tokenized;

/// An on-chain transaction that settled a solution.
#[derive(Debug, Clone)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The associated auction id.
    pub auction_id: domain::auction::Id,
    /// The block number of the block that contains the transaction.
    pub block: eth::BlockNo,
    /// The timestamp of the block that contains the transaction.
    pub timestamp: u32,
    /// The gas used by the transaction.
    pub gas: eth::Gas,
    /// The effective gas price of the transaction.
    pub gas_price: eth::EffectiveGasPrice,
    /// Encoded trades that were settled by the transaction.
    pub trades: Vec<EncodedTrade>,
}

impl Transaction {
    pub fn try_new(
        transaction: &eth::Transaction,
        domain_separator: &eth::DomainSeparator,
        settlement_contract: eth::Address,
    ) -> Result<Self, Error> {
        // find trace call to settlement contract
        let calldata = transaction
            .trace_calls
            .iter()
            .find(|trace| is_settlement_trace(trace, settlement_contract))
            .map(|trace| trace.input.clone())
            // all transactions emitting settlement events should have a /settle call, 
            // otherwise it's an execution client bug
            .ok_or(Error::MissingCalldata)?;

        /// Number of bytes that may be appended to the calldata to store an
        /// auction id.
        const META_DATA_LEN: usize = 8;

        let (data, metadata) = calldata.0.split_at(
            calldata
                .0
                .len()
                .checked_sub(META_DATA_LEN)
                // should contain at least 4 bytes for function selector and 8 bytes for auction id
                .ok_or(Error::MissingCalldata)?,
        );
        let metadata: Option<[u8; META_DATA_LEN]> = metadata.try_into().ok();
        let auction_id = metadata
            .map(crate::domain::auction::Id::from_be_bytes)
            .ok_or(Error::MissingAuctionId)?;
        Ok(Self {
            hash: transaction.hash,
            auction_id,
            block: transaction.block,
            timestamp: transaction.timestamp,
            gas: transaction.gas,
            gas_price: transaction.gas_price,
            trades: {
                let tokenized::Tokenized {
                    tokens,
                    clearing_prices,
                    trades: decoded_trades,
                    interactions: _interactions,
                } = tokenized::Tokenized::try_new(&crate::util::Bytes(data.to_vec()))?;

                let mut trades = Vec::with_capacity(decoded_trades.len());
                for trade in decoded_trades {
                    let flags = tokenized::TradeFlags(trade.8);
                    let sell_token_index = trade.0.as_usize();
                    let buy_token_index = trade.1.as_usize();
                    let sell_token = tokens[sell_token_index];
                    let buy_token = tokens[buy_token_index];
                    let uniform_sell_token_index = tokens
                        .iter()
                        .position(|token| token == &sell_token)
                        .unwrap();
                    let uniform_buy_token_index =
                        tokens.iter().position(|token| token == &buy_token).unwrap();
                    trades.push(EncodedTrade {
                        uid: tokenized::order_uid(&trade, &tokens, domain_separator)
                            .map_err(Error::OrderUidRecover)?,
                        sell: eth::Asset {
                            token: sell_token.into(),
                            amount: trade.3.into(),
                        },
                        buy: eth::Asset {
                            token: buy_token.into(),
                            amount: trade.4.into(),
                        },
                        side: flags.side(),
                        receiver: trade.2.into(),
                        valid_to: trade.5,
                        app_data: domain::auction::order::AppDataHash(trade.6 .0),
                        fee_amount: trade.7.into(),
                        sell_token_balance: flags.sell_token_balance().into(),
                        buy_token_balance: flags.buy_token_balance().into(),
                        partially_fillable: flags.partially_fillable(),
                        signature: (boundary::Signature::from_bytes(
                            flags.signing_scheme(),
                            &trade.10 .0,
                        )
                        .map_err(Error::SignatureRecover)?)
                        .into(),
                        executed: trade.9.into(),
                        prices: Prices {
                            uniform: ClearingPrices {
                                sell: clearing_prices[uniform_sell_token_index].into(),
                                buy: clearing_prices[uniform_buy_token_index].into(),
                            },
                            custom: ClearingPrices {
                                sell: clearing_prices[sell_token_index].into(),
                                buy: clearing_prices[buy_token_index].into(),
                            },
                        },
                    })
                }
                trades
            },
        })
    }
}

fn is_settlement_trace(trace: &eth::TraceCall, settlement_contract: eth::Address) -> bool {
    static SETTLE_FUNCTION_SELECTOR: LazyLock<[u8; 4]> = LazyLock::new(|| {
        let abi = &contracts::GPv2Settlement::raw_contract().interface.abi;
        abi.function("settle").unwrap().selector()
    });
    trace.to == settlement_contract && trace.input.0.starts_with(&*SETTLE_FUNCTION_SELECTOR)
}

/// Trade containing onchain observable data specific to a settlement
/// transaction.
#[derive(Debug, Clone)]
pub struct EncodedTrade {
    pub uid: domain::OrderUid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: order::Side,
    pub receiver: eth::Address,
    pub valid_to: u32,
    pub app_data: order::AppDataHash,
    pub fee_amount: eth::TokenAmount,
    pub sell_token_balance: order::SellTokenSource,
    pub buy_token_balance: order::BuyTokenDestination,
    pub partially_fillable: bool,
    pub signature: order::Signature,
    pub executed: order::TargetAmount,
    pub prices: Prices,
}

#[derive(Debug, Copy, Clone)]
pub struct Prices {
    pub uniform: ClearingPrices,
    /// Adjusted uniform prices to account for fees (gas cost and protocol fees)
    pub custom: ClearingPrices,
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("settle calldata must exist for all transactions emitting settlement event")]
    MissingCalldata,
    #[error("missing auction id")]
    MissingAuctionId,
    #[error(transparent)]
    Decoding(#[from] tokenized::error::Decoding),
    #[error("failed to recover order uid {0}")]
    OrderUidRecover(tokenized::error::Uid),
    #[error("failed to recover signature {0}")]
    SignatureRecover(#[source] anyhow::Error),
}

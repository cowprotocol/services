use {
    crate::{
        boundary,
        domain::{self, auction::order, blockchain},
    },
    alloy::{eips::BlockId, sol_types::SolCall},
    contracts::{GPv2AllowListAuthentication, GPv2Settlement},
    eth_domain_types as eth,
    std::collections::HashSet,
};

mod tokenized;

/// The following trait allows to implement custom solver authentication logic
/// for transactions.
#[async_trait::async_trait]
pub trait Authenticator {
    /// Determines whether the provided address is an authenticated solver.
    async fn is_valid_solver(
        &self,
        prospective_solver: eth::Address,
        block: BlockId,
    ) -> Result<bool, Error>;
}

#[async_trait::async_trait]
impl Authenticator for GPv2AllowListAuthentication::Instance {
    async fn is_valid_solver(
        &self,
        prospective_solver: eth::Address,
        block: BlockId,
    ) -> Result<bool, Error> {
        // It's technically possible that some time passes between the
        // transaction happening and us indexing it. If the transaction
        // was malicious and the solver got deny listed by the circuit
        // breaker because of it we wouldn't find an eligible caller in
        // the callstack. To avoid this case the underlying call needs
        // to happen on the same block the transaction happened.
        Ok(self
            .isSolver(prospective_solver)
            .block(block)
            .call()
            .await
            .map_err(Error::Authentication)?)
    }
}

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
    /// The solver (submission address)
    pub solver: eth::Address,
    /// Encoded trades that were settled by the transaction.
    pub trades: Vec<EncodedTrade>,
}

impl Transaction {
    pub async fn try_new(
        transaction: &blockchain::Transaction,
        domain_separator: &eth::DomainSeparator,
        settlement_contract: eth::Address,
        weth: eth::WrappedNativeToken,
        authenticator: &impl Authenticator,
    ) -> Result<Self, Error> {
        // Find trace call to settlement contract
        let (calldata, callers) = find_settlement_trace_and_callers(&transaction.trace_calls, settlement_contract)
            .map(|(trace, path)| (trace.input.clone(), path.clone()))
            // All transactions emitting settlement events should have a /settle call,
            // otherwise it's an execution client bug
            .ok_or(Error::MissingCalldata)?;

        // Find solver (submission address)
        // In cases of solvers using EOA to submit solutions, the address is the
        // sender of the transaction. In cases of solvers using a smart
        // contract to submit solutions, the address is deduced from the
        // calldata.
        let block = BlockId::from(transaction.block.0);
        let solver = find_solver_address(authenticator, callers, block).await?;

        /// Number of bytes that may be appended to the calldata to store an
        /// auction id.
        const META_DATA_LEN: usize = 8;

        let (data, metadata) = calldata.0.split_at(
            calldata
                .0
                .len()
                .checked_sub(META_DATA_LEN)
                // should contain at META_DATA_LEN bytes for auction id
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
            solver: solver.ok_or(Error::MissingSolver)?,
            trades: {
                let GPv2Settlement::GPv2Settlement::settleCall {
                    trades: decoded_trades,
                    tokens,
                    clearingPrices: clearing_prices,
                    ..
                } = GPv2Settlement::GPv2Settlement::settleCall::abi_decode(data)?;

                // Custom prices follow the uniform ones, so the lowest trade
                // index marks the end of the uniform prices.
                let uniform_len = decoded_trades
                    .iter()
                    .flat_map(|trade| [trade.sellTokenIndex, trade.buyTokenIndex])
                    .min()
                    .map_or(0, |index| {
                        usize::try_from(index).expect("SC was able to look up this index")
                    });

                let mut trades = Vec::with_capacity(decoded_trades.len());
                for trade in decoded_trades {
                    let flags = tokenized::TradeFlags(trade.flags);
                    let sell_token_index = usize::try_from(trade.sellTokenIndex)
                        .expect("SC was able to look up this index");
                    let buy_token_index = usize::try_from(trade.buyTokenIndex)
                        .expect("SC was able to look up this index");
                    let sell_token = tokens[sell_token_index];
                    let buy_token = tokens[buy_token_index];
                    let uniform_sell_token_index =
                        uniform_token_index(&tokens, uniform_len, sell_token, weth);
                    let uniform_buy_token_index =
                        uniform_token_index(&tokens, uniform_len, buy_token, weth);
                    trades.push(EncodedTrade {
                        uid: tokenized::order_uid(&trade, &tokens, domain_separator)
                            .map_err(Error::OrderUidRecover)?,
                        sell: eth::Asset {
                            token: sell_token.into(),
                            amount: trade.sellAmount.into(),
                        },
                        buy: eth::Asset {
                            token: buy_token.into(),
                            amount: trade.buyAmount.into(),
                        },
                        side: flags.side(),
                        receiver: trade.receiver,
                        valid_to: trade.validTo,
                        app_data: domain::auction::order::AppDataHash(trade.appData.into()),
                        fee_amount: trade.feeAmount.into(),
                        sell_token_balance: flags.sell_token_balance().into(),
                        buy_token_balance: flags.buy_token_balance().into(),
                        partially_fillable: flags.partially_fillable(),
                        signature: (boundary::Signature::from_bytes(
                            flags.signing_scheme(),
                            trade.signature.as_ref(),
                        )
                        .map_err(Error::SignatureRecover)?)
                        .into(),
                        executed: trade.executedAmount.into(),
                        prices: Prices {
                            uniform: ClearingPrices {
                                sell: clearing_prices[uniform_sell_token_index],
                                buy: clearing_prices[uniform_buy_token_index],
                            },
                            custom: ClearingPrices {
                                sell: clearing_prices[sell_token_index],
                                buy: clearing_prices[buy_token_index],
                            },
                        },
                    })
                }
                trades
            },
        })
    }
}

/// Index of `token`'s uniform price in `tokens[..uniform_len]`, with ETH and
/// WETH standing in for each other. Falls back to the first occurrence.
fn uniform_token_index(
    tokens: &[eth::Address],
    uniform_len: usize,
    token: eth::Address,
    weth: eth::WrappedNativeToken,
) -> usize {
    let uniform = &tokens[..uniform_len];
    let erc20 = |token: eth::Address| eth::TokenAddress::from(token).as_erc20(weth);
    uniform
        .iter()
        .position(|t| *t == token)
        .or_else(|| uniform.iter().position(|t| erc20(*t) == erc20(token)))
        .or_else(|| tokens.iter().position(|t| *t == token))
        .expect("token was read from `tokens`")
}

fn find_settlement_trace_and_callers(
    call_frame: &blockchain::CallFrame,
    settlement_contract: eth::Address,
) -> Option<(&blockchain::CallFrame, Vec<eth::Address>)> {
    // Use a queue (VecDeque) to keep track of frames to process
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((call_frame, vec![call_frame.from]));

    while let Some((call_frame, callers_so_far)) = queue.pop_front() {
        if is_settlement_trace(call_frame, settlement_contract) {
            return Some((call_frame, callers_so_far));
        }
        // Add all nested calls to the queue with the updated caller
        for sub_call in &call_frame.calls {
            let mut new_callers = callers_so_far.clone();
            new_callers.push(sub_call.from);
            queue.push_back((sub_call, new_callers));
        }
    }

    None
}

fn is_settlement_trace(trace: &blockchain::CallFrame, settlement_contract: eth::Address) -> bool {
    let settle_selector = &GPv2Settlement::GPv2Settlement::settleCall::SELECTOR;
    trace.to.unwrap_or_default() == settlement_contract
        && trace.input.0.starts_with(settle_selector)
}

async fn find_solver_address(
    authenticator: &impl Authenticator,
    callers: Vec<eth::Address>,
    block: BlockId,
) -> Result<Option<eth::Address>, Error> {
    let mut checked_callers = HashSet::new();
    for caller in &callers {
        if !checked_callers.insert(caller) {
            // skip caller if we already checked it
            continue;
        }

        if authenticator
            .is_valid_solver(caller.0.into(), block)
            .await?
        {
            return Ok(Some(*caller));
        }
    }
    Ok(None)
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
    #[error("solver address must be deductible from calldata")]
    MissingSolver,
    #[error("missing auction id")]
    MissingAuctionId,
    #[error(transparent)]
    Decoding(#[from] alloy::sol_types::Error),
    #[error("failed to recover order uid {0}")]
    OrderUidRecover(tokenized::error::Uid),
    #[error("failed to recover signature {0}")]
    SignatureRecover(#[source] anyhow::Error),
    #[error("failed to check authentication {0}")]
    Authentication(#[source] alloy::contract::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_token_index_treats_eth_and_weth_as_equivalent() {
        let [a, b, weth] = [1, 2, 3].map(eth::Address::repeat_byte);
        let eth = *eth::ETH_TOKEN;
        let index = |tokens: &[eth::Address], uniform_len, token| {
            uniform_token_index(tokens, uniform_len, token, weth.into())
        };

        // An explicit uniform ETH price wins over WETH.
        assert_eq!(index(&[a, weth, eth, a, eth], 3, eth), 2);
        // ETH falls back to WETH, also for a second trade buying ETH.
        assert_eq!(index(&[a, weth, a, eth, b, eth], 2, eth), 1);
        // WETH falls back to ETH.
        assert_eq!(index(&[eth, a, weth, a], 1, weth), 0);
        // Without a uniform section the first occurrence is used.
        assert_eq!(index(&[a, weth, a, eth], 0, eth), 3);
        // Other tokens never resolve to ETH or WETH.
        assert_eq!(index(&[weth, a, b, a], 1, a), 1);
    }
}

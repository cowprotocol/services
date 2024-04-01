use {
    crate::domain::{self, eth},
    app_data::AppDataHash,
    ethcontract::{Address, Bytes, U256},
};

// Original type for input of `GPv2Settlement.settle` function.
pub(super) type Settlement = (
    Vec<Token>,
    Vec<ClearingPrice>,
    Vec<Trade>,
    [Vec<Interaction>; 3],
);

pub(super) type Token = Address;
pub(super) type ClearingPrice = U256;
pub(super) type Trade = (
    U256,            // sellTokenIndex
    U256,            // buyTokenIndex
    Address,         // receiver
    U256,            // sellAmount
    U256,            // buyAmount
    u32,             // validTo
    Bytes<[u8; 32]>, // appData
    U256,            // feeAmount
    U256,            // flags
    U256,            // executedAmount
    Bytes<Vec<u8>>,  // signature
);
pub(super) type Interaction = (Address, U256, Bytes<Vec<u8>>);

/// Recover order uid from order data and signature
pub fn order_uid(
    trade: &Trade,
    tokens: &[Token],
    domain_separator: &eth::DomainSeparator,
) -> Result<domain::OrderUid, Error> {
    let flags = super::TradeFlags(trade.8);
    let signature = crate::boundary::Signature::from_bytes(flags.signing_scheme(), &trade.10 .0)
        .map_err(Error::Signature)?;

    let order = model::order::OrderData {
        sell_token: tokens[trade.0.as_u64() as usize],
        buy_token: tokens[trade.1.as_u64() as usize],
        receiver: Some(trade.2),
        sell_amount: trade.3,
        buy_amount: trade.4,
        valid_to: trade.5,
        app_data: AppDataHash(trade.6 .0),
        fee_amount: trade.7,
        kind: match flags.side() {
            domain::auction::order::Side::Buy => model::order::OrderKind::Buy,
            domain::auction::order::Side::Sell => model::order::OrderKind::Sell,
        },
        partially_fillable: flags.partially_fillable(),
        sell_token_balance: flags.sell_token_balance(),
        buy_token_balance: flags.buy_token_balance(),
    };
    let domain_separator = crate::boundary::DomainSeparator(domain_separator.0);
    let owner = signature
        .recover_owner(&trade.10 .0, &domain_separator, &order.hash_struct())
        .map_err(Error::RecoverOwner)?;
    Ok(order.uid(&domain_separator, &owner).into())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bad signature {0}")]
    Signature(anyhow::Error),
    #[error("recover owner {0}")]
    RecoverOwner(anyhow::Error),
}

use {
    crate::{
        boundary,
        domain::{self, auction::order, eth},
    },
    app_data::AppDataHash,
    ethcontract::{common::FunctionExt, tokens::Tokenize, Address, Bytes, U256},
};

// Original type for input of `GPv2Settlement.settle` function.
pub struct Tokenized {
    pub tokens: Vec<Address>,
    pub clearing_prices: Vec<eth::TokenAmount>,
    pub trades: Vec<Trade>,
    pub interactions: [Vec<Interaction>; 3],
}

impl Tokenized {
    pub fn new(calldata: &eth::Calldata) -> Result<Self, error::Decoding> {
        let function = contracts::GPv2Settlement::raw_contract()
            .interface
            .abi
            .function("settle")
            .unwrap();
        let data = calldata
            .0
            .strip_prefix(&function.selector())
            .ok_or(error::Decoding::InvalidSelector)?;
        let tokenized = function
            .decode_input(data)
            .map_err(error::Decoding::Ethabi)?;
        let (tokens, clearing_prices, trades, interactions) =
            <Solution>::from_token(web3::ethabi::Token::Tuple(tokenized))
                .map_err(error::Decoding::Tokenizing)?;
        Ok(Self {
            tokens,
            clearing_prices: clearing_prices.into_iter().map(Into::into).collect(),
            trades,
            interactions,
        })
    }
}

type Token = Address;
type Trade = (
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
type Interaction = (Address, U256, Bytes<Vec<u8>>);
type Solution = (Vec<Address>, Vec<U256>, Vec<Trade>, [Vec<Interaction>; 3]);

/// Recover order uid from order data and signature
pub fn order_uid(
    trade: &Trade,
    tokens: &[Token],
    domain_separator: &eth::DomainSeparator,
) -> Result<domain::OrderUid, error::Uid> {
    let flags = TradeFlags(trade.8);
    let signature = crate::boundary::Signature::from_bytes(flags.signing_scheme(), &trade.10 .0)
        .map_err(error::Uid::Signature)?;

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
        .map_err(error::Uid::RecoverOwner)?;
    Ok(order.uid(&domain_separator, &owner).into())
}

/// Trade flags are encoded in a 256-bit integer field. For more information on
/// how flags are encoded see:
/// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/libraries/GPv2Trade.sol#L58-L94>
#[derive(Debug, PartialEq, Eq)]
pub struct TradeFlags(pub U256);

impl TradeFlags {
    fn as_u8(&self) -> u8 {
        self.0.byte(0)
    }

    pub fn side(&self) -> order::Side {
        if self.as_u8() & 0b1 == 0 {
            order::Side::Sell
        } else {
            order::Side::Buy
        }
    }

    pub fn partially_fillable(&self) -> bool {
        self.as_u8() & 0b10 != 0
    }

    pub fn sell_token_balance(&self) -> boundary::SellTokenSource {
        if self.as_u8() & 0x08 == 0 {
            boundary::SellTokenSource::Erc20
        } else if self.as_u8() & 0x04 == 0 {
            boundary::SellTokenSource::External
        } else {
            boundary::SellTokenSource::Internal
        }
    }

    pub fn buy_token_balance(&self) -> boundary::BuyTokenDestination {
        if self.as_u8() & 0x10 == 0 {
            boundary::BuyTokenDestination::Erc20
        } else {
            boundary::BuyTokenDestination::Internal
        }
    }

    pub fn signing_scheme(&self) -> boundary::SigningScheme {
        match (self.as_u8() >> 5) & 0b11 {
            0b00 => boundary::SigningScheme::Eip712,
            0b01 => boundary::SigningScheme::EthSign,
            0b10 => boundary::SigningScheme::Eip1271,
            0b11 => boundary::SigningScheme::PreSign,
            _ => unreachable!(),
        }
    }
}

impl From<U256> for TradeFlags {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum Uid {
        #[error("bad signature {0}")]
        Signature(anyhow::Error),
        #[error("recover owner {0}")]
        RecoverOwner(anyhow::Error),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Decoding {
        #[error("transaction calldata is not a settlement")]
        InvalidSelector,
        #[error("unable to decode settlement calldata: {0}")]
        Ethabi(web3::ethabi::Error),
        #[error("unable to tokenize calldata into expected format: {0}")]
        Tokenizing(ethcontract::tokens::Error),
    }
}

use {
    crate::{
        boundary,
        domain::{self, auction::order, eth},
    },
    alloy::primitives::U256,
    app_data::AppDataHash,
    contracts::GPv2Settlement,
};

/// Recover order uid from order data and signature
pub fn order_uid(
    trade: &GPv2Settlement::GPv2Trade::Data,
    tokens: &[alloy::primitives::Address],
    domain_separator: &eth::DomainSeparator,
) -> Result<domain::OrderUid, error::Uid> {
    let flags = TradeFlags(trade.flags);
    let signature =
        crate::boundary::Signature::from_bytes(flags.signing_scheme(), &trade.signature.0)
            .map_err(error::Uid::Signature)?;

    let order = model::order::OrderData {
        sell_token: tokens
            [usize::try_from(trade.sellTokenIndex).expect("SC was able to look up this index")],
        buy_token: tokens
            [usize::try_from(trade.buyTokenIndex).expect("SC was able to look up this index")],
        receiver: Some(trade.receiver),
        sell_amount: trade.sellAmount,
        buy_amount: trade.buyAmount,
        valid_to: trade.validTo,
        app_data: AppDataHash(trade.appData.0),
        fee_amount: trade.feeAmount,
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
        .recover_owner(&trade.signature.0, &domain_separator, &order.hash_struct())
        .map_err(error::Uid::RecoverOwner)?;
    Ok(order.uid(&domain_separator, owner).into())
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
}

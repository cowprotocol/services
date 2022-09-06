use anyhow::{ensure, Result};
use database::orders::{
    BuyTokenDestination as DbBuyTokenDestination, OrderKind as DbOrderKind,
    SellTokenSource as DbSellTokenSource, SigningScheme as DbSigningScheme,
};
use model::{
    order::{BuyTokenDestination, OrderKind, SellTokenSource},
    signature::SigningScheme,
};
use num::{rational::Ratio, BigInt, BigRational};
use primitive_types::U256;

pub fn order_kind_from(kind: DbOrderKind) -> OrderKind {
    match kind {
        DbOrderKind::Buy => OrderKind::Buy,
        DbOrderKind::Sell => OrderKind::Sell,
    }
}

pub fn sell_token_source_into(source: SellTokenSource) -> DbSellTokenSource {
    match source {
        SellTokenSource::Erc20 => DbSellTokenSource::Erc20,
        SellTokenSource::Internal => DbSellTokenSource::Internal,
        SellTokenSource::External => DbSellTokenSource::External,
    }
}

pub fn sell_token_source_from(source: DbSellTokenSource) -> SellTokenSource {
    match source {
        DbSellTokenSource::Erc20 => SellTokenSource::Erc20,
        DbSellTokenSource::Internal => SellTokenSource::Internal,
        DbSellTokenSource::External => SellTokenSource::External,
    }
}

pub fn buy_token_destination_into(destination: BuyTokenDestination) -> DbBuyTokenDestination {
    match destination {
        BuyTokenDestination::Erc20 => DbBuyTokenDestination::Erc20,
        BuyTokenDestination::Internal => DbBuyTokenDestination::Internal,
    }
}

pub fn buy_token_destination_from(destination: DbBuyTokenDestination) -> BuyTokenDestination {
    match destination {
        DbBuyTokenDestination::Erc20 => BuyTokenDestination::Erc20,
        DbBuyTokenDestination::Internal => BuyTokenDestination::Internal,
    }
}

pub fn signing_scheme_into(scheme: SigningScheme) -> DbSigningScheme {
    match scheme {
        SigningScheme::Eip712 => DbSigningScheme::Eip712,
        SigningScheme::EthSign => DbSigningScheme::EthSign,
        SigningScheme::Eip1271 => DbSigningScheme::Eip1271,
        SigningScheme::PreSign => DbSigningScheme::PreSign,
    }
}

pub fn signing_scheme_from(scheme: DbSigningScheme) -> SigningScheme {
    match scheme {
        DbSigningScheme::Eip712 => SigningScheme::Eip712,
        DbSigningScheme::EthSign => SigningScheme::EthSign,
        DbSigningScheme::Eip1271 => SigningScheme::Eip1271,
        DbSigningScheme::PreSign => SigningScheme::PreSign,
    }
}

// Convenience:

pub trait RatioExt<T> {
    fn new_checked(numerator: T, denominator: T) -> Result<Ratio<T>>;
}

impl<T: num::Integer + Clone> RatioExt<T> for Ratio<T> {
    fn new_checked(numerator: T, denominator: T) -> Result<Ratio<T>> {
        ensure!(
            !denominator.is_zero(),
            "Cannot create Ratio with 0 denominator"
        );
        Ok(Ratio::new(numerator, denominator))
    }
}

pub trait U256Ext: Sized {
    fn to_big_int(&self) -> BigInt;
    fn to_big_rational(&self) -> BigRational;

    fn checked_ceil_div(&self, other: &Self) -> Option<Self>;
    fn ceil_div(&self, other: &Self) -> Self;
}

impl U256Ext for U256 {
    fn to_big_int(&self) -> BigInt {
        number_conversions::u256_to_big_int(self)
    }
    fn to_big_rational(&self) -> BigRational {
        number_conversions::u256_to_big_rational(self)
    }

    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(1.into())?)?
            .checked_div(*other)
    }
    fn ceil_div(&self, other: &Self) -> Self {
        self.checked_ceil_div(other)
            .expect("ceiling division arithmetic error")
    }
}

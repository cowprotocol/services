use database::orders::{
    BuyTokenDestination as DbBuyTokenDestination, OrderKind as DbOrderKind,
    SellTokenSource as DbSellTokenSource, SigningScheme as DbSigningScheme,
};
use model::{
    order::{BuyTokenDestination, OrderKind, SellTokenSource},
    signature::SigningScheme,
};

pub fn order_kind_into(kind: OrderKind) -> DbOrderKind {
    match kind {
        OrderKind::Buy => DbOrderKind::Buy,
        OrderKind::Sell => DbOrderKind::Sell,
    }
}

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

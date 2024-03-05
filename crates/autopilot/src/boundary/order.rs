use {crate::domain, shared::remaining_amounts};

pub fn to_domain(
    order: model::order::Order,
    protocol_fees: Vec<domain::fee::Policy>,
) -> domain::Order {
    let remaining_order = remaining_amounts::Order::from(order.clone());
    let order_is_untouched = remaining_order.executed_amount.is_zero();

    domain::Order {
        uid: order.metadata.uid.into(),
        sell_token: order.data.sell_token,
        buy_token: order.data.buy_token,
        sell_amount: order.data.sell_amount,
        buy_amount: order.data.buy_amount,
        user_fee: order.data.fee_amount,
        protocol_fees,
        valid_to: order.data.valid_to,
        side: order.data.kind.into(),
        receiver: order.data.receiver,
        owner: order.metadata.owner,
        partially_fillable: order.data.partially_fillable,
        executed: remaining_order.executed_amount,
        pre_interactions: order_is_untouched
            .then(|| order.interactions.pre.into_iter().map(Into::into).collect())
            .unwrap_or_default(),
        post_interactions: order
            .interactions
            .post
            .into_iter()
            .map(Into::into)
            .collect(),
        sell_token_balance: order.data.sell_token_balance.into(),
        buy_token_balance: order.data.buy_token_balance.into(),
        class: order.metadata.class.into(),
        app_data: order.data.app_data.into(),
        signature: order.signature.into(),
    }
}

/// Recover order uid from order data and signature
pub fn order_uid(
    trade: &domain::settlement::coded::tokenized::Trade,
    tokens: &[domain::settlement::coded::tokenized::Token],
    domain_separator: &domain::eth::DomainSeparator,
) -> Result<domain::OrderUid, Error> {
    let flags = domain::settlement::coded::TradeFlags(trade.8);
    let signature = crate::boundary::Signature::from_bytes(flags.signing_scheme(), &trade.10 .0)
        .map_err(Error::Signature)?;

    let order = model::order::OrderData {
        sell_token: tokens[trade.0.as_u64() as usize],
        buy_token: tokens[trade.1.as_u64() as usize],
        sell_amount: trade.3,
        buy_amount: trade.4,
        valid_to: trade.5,
        app_data: crate::boundary::AppDataHash(trade.6 .0),
        fee_amount: trade.7,
        kind: match flags.order_kind() {
            domain::auction::order::Side::Buy => model::order::OrderKind::Buy,
            domain::auction::order::Side::Sell => model::order::OrderKind::Sell,
        },
        partially_fillable: flags.partially_fillable(),
        receiver: Some(trade.2),
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

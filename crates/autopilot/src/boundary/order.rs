use {
    crate::{domain, domain::eth},
    shared::remaining_amounts,
};

pub fn to_domain(
    order: model::order::Order,
    protocol_fees: Vec<domain::fee::Policy>,
    quote: Option<domain::Quote>,
) -> domain::Order {
    let remaining_order = remaining_amounts::Order::from(order.clone());
    let order_is_untouched = remaining_order.executed_amount.is_zero();

    domain::Order {
        uid: order.metadata.uid.into(),
        sell: eth::Asset {
            token: order.data.sell_token.into(),
            amount: order.data.sell_amount.into(),
        },
        buy: eth::Asset {
            token: order.data.buy_token.into(),
            amount: order.data.buy_amount.into(),
        },
        protocol_fees,
        created: Some(u32::try_from(order.metadata.creation_date.timestamp()).unwrap_or(u32::MIN)),
        valid_to: order.data.valid_to,
        side: order.data.kind.into(),
        receiver: order.data.receiver.map(Into::into),
        owner: order.metadata.owner.into(),
        partially_fillable: order.data.partially_fillable,
        executed: remaining_order.executed_amount.into(),
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
        quote,
    }
}

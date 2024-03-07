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

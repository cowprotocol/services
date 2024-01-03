use {
    crate::domain,
    model::{
        interaction::InteractionData,
        order::{OrderClass, OrderKind},
    },
    shared::remaining_amounts,
};

pub fn to_domain(
    order: model::order::Order,
    quote: Option<&domain::Quote>,
    fee_policies: &domain::fee::Policies,
) -> domain::Order {
    let remaining_order = remaining_amounts::Order::from(order.clone());
    let order_is_untouched = remaining_order.executed_amount.is_zero();
    let fee_policies = match quote {
        None => vec![],
        Some(quote) => fee_policies.get(&order, quote),
    };

    domain::Order {
        uid: order.metadata.uid,
        sell_token: order.data.sell_token,
        buy_token: order.data.buy_token,
        sell_amount: order.data.sell_amount,
        buy_amount: order.data.buy_amount,
        solver_fee: order.metadata.full_fee_amount,
        user_fee: order.data.fee_amount,
        valid_to: order.data.valid_to,
        kind: order.data.kind.into(),
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
        sell_token_balance: order.data.sell_token_balance,
        buy_token_balance: order.data.buy_token_balance,
        class: order.metadata.class.into(),
        app_data: order.data.app_data,
        signature: order.signature,
        fee_policies,
    }
}

impl From<OrderClass> for domain::auction::order::Class {
    fn from(class: OrderClass) -> Self {
        match class {
            OrderClass::Market => Self::Market,
            OrderClass::Liquidity => Self::Liquidity,
            OrderClass::Limit(_) => Self::Limit,
        }
    }
}

impl From<OrderKind> for domain::auction::order::OrderKind {
    fn from(kind: OrderKind) -> Self {
        match kind {
            OrderKind::Buy => Self::Buy,
            OrderKind::Sell => Self::Sell,
        }
    }
}

impl From<InteractionData> for domain::auction::order::Interaction {
    fn from(interaction: InteractionData) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            call_data: interaction.call_data,
        }
    }
}

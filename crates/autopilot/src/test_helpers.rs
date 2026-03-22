use {crate::domain, alloy::primitives::Address, eth_domain_types as eth};

pub fn test_order(uid_byte: u8, amount: u8) -> domain::Order {
    domain::Order {
        uid: domain::OrderUid([uid_byte; 56]),
        sell: eth::Asset {
            token: Address::repeat_byte(uid_byte).into(),
            amount: eth::TokenAmount::from(eth::U256::from(u128::from(amount))),
        },
        buy: eth::Asset {
            token: Address::repeat_byte(uid_byte.saturating_add(1)).into(),
            amount: eth::TokenAmount::from(eth::U256::from(u128::from(amount.saturating_add(1)))),
        },
        protocol_fees: Vec::new(),
        side: domain::auction::order::Side::Sell,
        created: u32::from(uid_byte),
        valid_to: u32::from(uid_byte) + 100,
        receiver: None,
        owner: Address::repeat_byte(uid_byte.saturating_add(2)),
        partially_fillable: false,
        executed: domain::auction::order::TargetAmount(eth::U256::ZERO),
        pre_interactions: Vec::new(),
        post_interactions: Vec::new(),
        sell_token_balance: domain::auction::order::SellTokenSource::Erc20,
        buy_token_balance: domain::auction::order::BuyTokenDestination::Erc20,
        app_data: domain::auction::order::AppDataHash([uid_byte; 32]),
        signature: domain::auction::order::Signature::PreSign,
        quote: None,
    }
}

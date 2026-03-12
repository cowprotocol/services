use {
    crate::{
        domain::{auction, dex::*, eth::*},
        infra::dex::bitget as bitget_dex,
    },
    alloy::primitives::address,
    std::{collections::HashMap, env},
};

#[ignore]
#[tokio::test]
// To run this test, set the following environment variables accordingly to your
// Bitget setup: BITGET_API_KEY, BITGET_API_SECRET
async fn swap_sell_regular() {
    let config = bitget_dex::Config {
        endpoint: reqwest::Url::parse(bitget_dex::DEFAULT_ENDPOINT).unwrap(),
        chain_id: crate::domain::eth::ChainId::Mainnet,
        credentials: bitget_dex::BitgetCredentialsConfig {
            api_key: env::var("BITGET_API_KEY").unwrap(),
            api_secret: env::var("BITGET_API_SECRET").unwrap(),
        },
        partner_code: "cowswap".to_string(),
        settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
        block_stream: None,
    };

    let order = Order {
        sell: TokenAddress::from(address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        buy: TokenAddress::from(address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        side: crate::domain::order::Side::Sell,
        amount: Amount::new(U256::from(1_000_000_000_000_000_000_u128)),
        owner: address!("0x6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
    };

    let slippage = Slippage::one_percent();

    let mut token_map = HashMap::new();
    token_map.insert(
        order.sell,
        auction::Token {
            decimals: Some(18),
            symbol: None,
            reference_price: None,
            available_balance: U256::ZERO,
            trusted: false,
        },
    );
    token_map.insert(
        order.buy,
        auction::Token {
            decimals: Some(6),
            symbol: None,
            reference_price: None,
            available_balance: U256::ZERO,
            trusted: false,
        },
    );
    let tokens = auction::Tokens(token_map);

    let bitget = bitget_dex::Bitget::try_new(config).unwrap();
    let swap_response = bitget.swap(&order, &slippage, &tokens).await;
    let swap = swap_response.unwrap();

    assert_eq!(swap.input.token, order.amount().token);
    assert_eq!(swap.input.amount, order.amount().amount);
    assert_eq!(swap.output.token, order.buy);
}

#[tokio::test]
async fn swap_buy_not_supported() {
    let config = bitget_dex::Config {
        endpoint: reqwest::Url::parse(bitget_dex::DEFAULT_ENDPOINT).unwrap(),
        chain_id: crate::domain::eth::ChainId::Mainnet,
        credentials: bitget_dex::BitgetCredentialsConfig {
            api_key: String::new(),
            api_secret: String::new(),
        },
        partner_code: "cowswap".to_string(),
        settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
        block_stream: None,
    };

    let order = Order {
        buy: TokenAddress::from(address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        sell: TokenAddress::from(address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        side: crate::domain::order::Side::Buy,
        amount: Amount::new(U256::from(1000000000_u128)),
        owner: address!("0x6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
    };

    let slippage = Slippage::one_percent();
    let tokens = auction::Tokens(HashMap::new());

    let bitget = bitget_dex::Bitget::try_new(config).unwrap();
    let swap_response = bitget.swap(&order, &slippage, &tokens).await;
    assert!(matches!(
        swap_response.unwrap_err(),
        bitget_dex::Error::OrderNotSupported
    ));
}

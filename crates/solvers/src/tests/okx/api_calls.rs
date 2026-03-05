use {
    crate::{
        domain::{dex::*, eth::*},
        infra::dex::okx as okx_dex,
    },
    alloy::primitives::address,
    std::env,
};

const DEFAULT_BUY_ORDERS_ENDPOINT: &str = "https://web3.okx.com/api/v5/dex/aggregator/";

#[ignore]
#[tokio::test]
// To run this test, set the following environment variables accordingly to your
// OKX setup: OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn swap_sell_regular() {
    let okx_config = okx_dex::Config {
        sell_orders_endpoint: reqwest::Url::parse(okx_dex::DEFAULT_SELL_ORDERS_ENDPOINT).unwrap(),
        buy_orders_endpoint: None,
        sell_orders_signature_base_url: None,
        buy_orders_signature_base_url: None,
        chain_id: crate::domain::eth::ChainId::Mainnet,
        okx_credentials: okx_dex::OkxCredentialsConfig {
            project_id: env::var("OKX_PROJECT_ID").unwrap(),
            api_key: env::var("OKX_API_KEY").unwrap(),
            api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
            api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
        },
        settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
        block_stream: None,
        price_impact_protection_percent: 1.0,
    };

    let order = Order {
        sell: TokenAddress::from(address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        buy: TokenAddress::from(address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        side: crate::domain::order::Side::Sell,
        amount: Amount::new(U256::from(100000000000000000_u128)),
        owner: address!("0x6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;
    let swap = swap_response.unwrap();

    assert_eq!(swap.input.token, order.amount().token);
    assert_eq!(swap.input.amount, order.amount().amount);
    assert_eq!(swap.output.token, order.buy);
    assert_eq!(
        swap.allowance.spender,
        address!("0x40aA958dd87FC8305b97f2BA922CDdCa374bcD7f")
    );
}

#[tokio::test]
async fn swap_buy_disabled() {
    let okx_config = okx_dex::Config {
        sell_orders_endpoint: reqwest::Url::parse(okx_dex::DEFAULT_SELL_ORDERS_ENDPOINT).unwrap(),
        buy_orders_endpoint: None,
        sell_orders_signature_base_url: None,
        buy_orders_signature_base_url: None,
        chain_id: crate::domain::eth::ChainId::Mainnet,
        okx_credentials: okx_dex::OkxCredentialsConfig {
            project_id: String::new(),
            api_key: String::new(),
            api_secret_key: String::new(),
            api_passphrase: String::new(),
        },
        settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
        block_stream: None,
        price_impact_protection_percent: 1.0,
    };

    let order = Order {
        buy: TokenAddress::from(address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        sell: TokenAddress::from(address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        side: crate::domain::order::Side::Buy,
        amount: Amount::new(U256::from(1000000000_u128)),
        owner: address!("0x6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;
    assert!(matches!(
        swap_response.unwrap_err(),
        crate::infra::dex::okx::Error::OrderNotSupported
    ));
}

#[ignore]
#[tokio::test]
// To run this test, set the following environment variables accordingly to your
// OKX setup: OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn swap_buy_enabled() {
    let okx_config = okx_dex::Config {
        sell_orders_endpoint: reqwest::Url::parse(okx_dex::DEFAULT_SELL_ORDERS_ENDPOINT).unwrap(),
        buy_orders_endpoint: Some(reqwest::Url::parse(DEFAULT_BUY_ORDERS_ENDPOINT).unwrap()),
        sell_orders_signature_base_url: None,
        buy_orders_signature_base_url: None,
        chain_id: crate::domain::eth::ChainId::Mainnet,
        okx_credentials: okx_dex::OkxCredentialsConfig {
            project_id: env::var("OKX_PROJECT_ID").unwrap(),
            api_key: env::var("OKX_API_KEY").unwrap(),
            api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
            api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
        },
        settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
        block_stream: None,
        price_impact_protection_percent: 1.0,
    };

    let order = Order {
        sell: TokenAddress::from(address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        buy: TokenAddress::from(address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        side: crate::domain::order::Side::Buy,
        amount: Amount::new(U256::from(1000000000_u128)),
        owner: address!("0x6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;
    let swap = swap_response.unwrap();

    assert_eq!(swap.output.token, order.amount().token);
    assert_eq!(swap.output.amount, order.amount().amount);
    assert_eq!(swap.input.token, order.sell);
    assert_eq!(
        swap.allowance.spender,
        address!("0x40aA958dd87FC8305b97f2BA922CDdCa374bcD7f")
    );
    // For buy orders, allowance should be U256::MAX
    assert_eq!(swap.allowance.amount.get(), U256::MAX);
}

#[ignore]
#[tokio::test]
// To run this test, set the following environment variables accordingly to your
// OKX setup: OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn swap_api_error() {
    let okx_config = okx_dex::Config {
        sell_orders_endpoint: reqwest::Url::parse(okx_dex::DEFAULT_SELL_ORDERS_ENDPOINT).unwrap(),
        buy_orders_endpoint: None,
        sell_orders_signature_base_url: None,
        buy_orders_signature_base_url: None,
        chain_id: crate::domain::eth::ChainId::Mainnet,
        okx_credentials: okx_dex::OkxCredentialsConfig {
            project_id: env::var("OKX_PROJECT_ID").unwrap(),
            api_key: env::var("OKX_API_KEY").unwrap(),
            api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
            api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
        },
        settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
        block_stream: None,
        price_impact_protection_percent: 1.0,
    };

    let order = Order {
        sell: TokenAddress::from(address!("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")),
        buy: TokenAddress::from(address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        side: crate::domain::order::Side::Sell,
        amount: Amount::new(U256::ZERO),
        owner: address!("6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;

    assert!(matches!(
        swap_response.unwrap_err(),
        crate::infra::dex::okx::Error::Api { .. }
    ));
}

#[ignore]
#[tokio::test]
// To run this test, set the following environment variables accordingly to your
// OKX setup: OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn swap_sell_insufficient_liquidity() {
    let okx_config = okx_dex::Config {
        sell_orders_endpoint: reqwest::Url::parse(okx_dex::DEFAULT_SELL_ORDERS_ENDPOINT).unwrap(),
        buy_orders_endpoint: None,
        sell_orders_signature_base_url: None,
        buy_orders_signature_base_url: None,
        chain_id: crate::domain::eth::ChainId::Mainnet,
        okx_credentials: okx_dex::OkxCredentialsConfig {
            project_id: env::var("OKX_PROJECT_ID").unwrap(),
            api_key: env::var("OKX_API_KEY").unwrap(),
            api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
            api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
        },
        settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
        block_stream: None,
        price_impact_protection_percent: 1.0,
    };

    let order = Order {
        sell: TokenAddress::from(address!("0xC8CD2BE653759aed7B0996315821AAe71e1FEAdF")),
        buy: TokenAddress::from(address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        side: crate::domain::order::Side::Sell,
        amount: Amount::new(U256::from(10000000000000_u128)),
        owner: address!("6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;

    assert!(matches!(
        swap_response.unwrap_err(),
        crate::infra::dex::okx::Error::NotFound
    ));
}

#[ignore]
#[tokio::test]
// To run this test, set the following environment variables accordingly to your
// OKX setup: OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn swap_buy_insufficient_liquidity() {
    let okx_config = okx_dex::Config {
        sell_orders_endpoint: reqwest::Url::parse(okx_dex::DEFAULT_SELL_ORDERS_ENDPOINT).unwrap(),
        buy_orders_endpoint: Some(reqwest::Url::parse(DEFAULT_BUY_ORDERS_ENDPOINT).unwrap()),
        sell_orders_signature_base_url: None,
        buy_orders_signature_base_url: None,
        chain_id: crate::domain::eth::ChainId::Mainnet,
        okx_credentials: okx_dex::OkxCredentialsConfig {
            project_id: env::var("OKX_PROJECT_ID").unwrap(),
            api_key: env::var("OKX_API_KEY").unwrap(),
            api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
            api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
        },
        settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
        block_stream: None,
        price_impact_protection_percent: 1.0,
    };

    let order = Order {
        sell: TokenAddress::from(address!("0xC8CD2BE653759aed7B0996315821AAe71e1FEAdF")),
        buy: TokenAddress::from(address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        side: crate::domain::order::Side::Buy,
        amount: Amount::new(U256::from(10000000000000_u128)),
        owner: address!("6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;

    assert!(matches!(
        swap_response.unwrap_err(),
        crate::infra::dex::okx::Error::NotFound
    ));
}

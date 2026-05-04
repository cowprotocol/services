use {
    crate::{
        domain::{auction, dex::*, eth::*},
        infra::dex::bitget as bitget_dex,
    },
    alloy::primitives::{Address, address},
    std::{collections::HashMap, env, time::Duration},
};

/// Pause between chain iterations to stay under Bitget's per-IP rate limit.
const RATE_LIMIT_PAUSE: Duration = Duration::from_secs(1);

struct TestCase {
    name: &'static str,
    chain_id: ChainId,
    sell_token: Address,
    buy_token: Address,
    sell_amount: u128,
    sell_decimals: u8,
    buy_decimals: u8,
}

const TEST_CASES: &[TestCase] = &[
    TestCase {
        name: "Mainnet: 0.1 WETH → USDC",
        chain_id: ChainId::Mainnet,
        sell_token: address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        buy_token: address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        sell_amount: 100_000_000_000_000_000,
        sell_decimals: 18,
        buy_decimals: 6,
    },
    TestCase {
        name: "Base: 0.1 WETH → USDC",
        chain_id: ChainId::Base,
        sell_token: address!("0x4200000000000000000000000000000000000006"),
        buy_token: address!("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        sell_amount: 100_000_000_000_000_000,
        sell_decimals: 18,
        buy_decimals: 6,
    },
    TestCase {
        name: "Arbitrum: 0.1 WETH → USDC",
        chain_id: ChainId::ArbitrumOne,
        sell_token: address!("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"),
        buy_token: address!("0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
        sell_amount: 100_000_000_000_000_000,
        sell_decimals: 18,
        buy_decimals: 6,
    },
    TestCase {
        name: "BNB: 1 WBNB → USDC",
        chain_id: ChainId::Bnb,
        sell_token: address!("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"),
        buy_token: address!("0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d"),
        sell_amount: 1_000_000_000_000_000_000,
        sell_decimals: 18,
        buy_decimals: 18,
    },
    TestCase {
        name: "Polygon: 100 WPOL → USDC",
        chain_id: ChainId::Polygon,
        sell_token: address!("0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270"),
        buy_token: address!("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
        sell_amount: 100_000_000_000_000_000_000,
        sell_decimals: 18,
        buy_decimals: 6,
    },
];

#[ignore]
#[tokio::test]
// To run this test, set the following environment variables accordingly to your
// Bitget setup: BITGET_API_KEY, BITGET_API_SECRET
async fn swap_sell_all_chains() {
    let api_key = env::var("BITGET_API_KEY").unwrap();
    let api_secret = env::var("BITGET_API_SECRET").unwrap();

    for tc in TEST_CASES {
        let config = bitget_dex::Config {
            endpoint: reqwest::Url::parse(bitget_dex::DEFAULT_ENDPOINT).unwrap(),
            chain_id: tc.chain_id,
            credentials: bitget_dex::BitgetCredentialsConfig {
                api_key: api_key.clone(),
                api_secret: api_secret.clone(),
            },
            partner_code: "cowswap".to_string(),
            settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
            block_stream: None,
            enable_buy_orders: false,
        };

        let order = Order {
            sell: TokenAddress::from(tc.sell_token),
            buy: TokenAddress::from(tc.buy_token),
            side: crate::domain::order::Side::Sell,
            amount: Amount::new(U256::from(tc.sell_amount)),
            owner: address!("0x6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
        };

        let slippage = Slippage::one_percent();

        let mut token_map = HashMap::new();
        token_map.insert(
            order.sell,
            auction::Token {
                decimals: Some(tc.sell_decimals),
                symbol: None,
                reference_price: None,
                available_balance: U256::ZERO,
                trusted: false,
            },
        );
        token_map.insert(
            order.buy,
            auction::Token {
                decimals: Some(tc.buy_decimals),
                symbol: None,
                reference_price: None,
                available_balance: U256::ZERO,
                trusted: false,
            },
        );
        let tokens = auction::Tokens(token_map);

        let bitget = bitget_dex::Bitget::try_new(config).unwrap();
        let swap = bitget
            .swap(&order, &slippage, &tokens)
            .await
            .unwrap_or_else(|e| panic!("[{}] swap failed: {e}", tc.name));

        assert_eq!(
            swap.input.token,
            order.amount().token,
            "[{}] input token mismatch",
            tc.name
        );
        assert_eq!(
            swap.input.amount,
            order.amount().amount,
            "[{}] input amount mismatch",
            tc.name
        );
        assert_eq!(
            swap.output.token, order.buy,
            "[{}] output token mismatch",
            tc.name
        );

        tokio::time::sleep(RATE_LIMIT_PAUSE).await;
    }
}

#[ignore]
#[tokio::test]
// To run this test, set the following environment variables accordingly to your
// Bitget setup: BITGET_API_KEY, BITGET_API_SECRET
async fn swap_buy_all_chains() {
    let api_key = env::var("BITGET_API_KEY").unwrap();
    let api_secret = env::var("BITGET_API_SECRET").unwrap();

    for tc in TEST_CASES {
        let config = bitget_dex::Config {
            endpoint: reqwest::Url::parse(bitget_dex::DEFAULT_ENDPOINT).unwrap(),
            chain_id: tc.chain_id,
            credentials: bitget_dex::BitgetCredentialsConfig {
                api_key: api_key.clone(),
                api_secret: api_secret.clone(),
            },
            partner_code: "cowswap".to_string(),
            settlement_contract: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
            block_stream: None,
            enable_buy_orders: true,
        };

        // Flip sell/buy so that we probe `buy_token = WETH` for a fixed amount,
        // mirroring the native-price probe shape.
        let buy_amount = tc.sell_amount;
        let order = Order {
            sell: TokenAddress::from(tc.buy_token),
            buy: TokenAddress::from(tc.sell_token),
            side: crate::domain::order::Side::Buy,
            amount: Amount::new(U256::from(buy_amount)),
            owner: address!("0x6f9ffea7370310cd0f890dfde5e0e061059dcfb8"),
        };

        let slippage = Slippage::one_percent();

        let mut token_map = HashMap::new();
        token_map.insert(
            order.sell,
            auction::Token {
                decimals: Some(tc.buy_decimals),
                symbol: None,
                reference_price: None,
                available_balance: U256::ZERO,
                trusted: false,
            },
        );
        token_map.insert(
            order.buy,
            auction::Token {
                decimals: Some(tc.sell_decimals),
                symbol: None,
                reference_price: None,
                available_balance: U256::ZERO,
                trusted: false,
            },
        );
        let tokens = auction::Tokens(token_map);

        let bitget = bitget_dex::Bitget::try_new(config).unwrap();
        let swap = bitget
            .swap(&order, &slippage, &tokens)
            .await
            .unwrap_or_else(|e| panic!("[{}] swap failed: {e}", tc.name));

        assert_eq!(
            swap.input.token, order.sell,
            "[{}] input token mismatch",
            tc.name
        );
        assert_eq!(
            swap.output.token, order.buy,
            "[{}] output token mismatch",
            tc.name
        );
        // Reverse-quote should produce a swap whose expected output meets or
        // exceeds the requested buy amount.
        assert!(
            swap.output.amount >= order.amount().amount,
            "[{}] expected output {} < requested {}",
            tc.name,
            swap.output.amount,
            order.amount().amount
        );

        tokio::time::sleep(RATE_LIMIT_PAUSE).await;
    }
}

#[tokio::test]
async fn swap_buy_disabled() {
    // With `enable_buy_orders` off (the default), buy orders must short-circuit
    // to `OrderNotSupported` without touching the API.
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
        enable_buy_orders: false,
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

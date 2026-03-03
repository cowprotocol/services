//! Module with a few functions duplicated from `shared` in order
//! to break cyclical dependencies and break off this crate.
use {
    alloy::primitives::{Address, B256, Bytes, U256},
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::{Signature, SigningScheme},
    },
    reqwest::{Client, ClientBuilder},
    std::{
        fmt::{Display, Formatter},
        time::Duration,
    },
    url::Url,
};

/// Join a path with a URL, ensuring that there is only one slash between them.
/// It doesn't matter if the URL ends with a slash or the path starts with one.
pub fn join_url(url: &Url, mut path: &str) -> Url {
    let mut url = url.to_string();
    while url.ends_with('/') {
        url.pop();
    }
    while path.starts_with('/') {
        path = &path[1..]
    }
    Url::parse(&format!("{url}/{path}")).unwrap()
}

/// anyhow errors are not clonable natively. This is a workaround that creates a
/// new anyhow error based on formatting the error with its inner sources
/// without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}

pub fn display_secret_option<T>(
    f: &mut Formatter<'_>,
    name: &str,
    option: Option<&T>,
) -> std::fmt::Result {
    display_option(f, name, &option.as_ref().map(|_| "SECRET"))
}

pub fn display_option(
    f: &mut Formatter<'_>,
    name: &str,
    option: &Option<impl Display>,
) -> std::fmt::Result {
    write!(f, "{name}: ")?;
    match option {
        Some(display) => writeln!(f, "{display}"),
        None => writeln!(f, "None"),
    }
}

pub type EncodedTrade = (
    U256,    // sellTokenIndex
    U256,    // buyTokenIndex
    Address, // receiver
    U256,    // sellAmount
    U256,    // buyAmount
    u32,     // validTo
    B256,    // appData
    U256,    // feeAmount
    U256,    // flags
    U256,    // executedAmount
    Bytes,   // signature
);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodedSettlement {
    pub tokens: Vec<Address>,
    pub clearing_prices: Vec<alloy::primitives::U256>,
    pub trades: Vec<EncodedTrade>,
    pub interactions: [Vec<EncodedInteraction>; 3],
}

pub type EncodedInteraction = (
    Address, // target
    U256,    // value
    Bytes,   // callData
);

/// Creates the data which the smart contract's `decodeTrade` expects.
pub fn encode_trade(
    order: &OrderData,
    signature: &Signature,
    owner: Address,
    sell_token_index: usize,
    buy_token_index: usize,
    executed_amount: U256,
) -> EncodedTrade {
    (
        U256::from(sell_token_index),
        U256::from(buy_token_index),
        order.receiver.unwrap_or(Address::ZERO),
        order.sell_amount,
        order.buy_amount,
        order.valid_to,
        B256::new(order.app_data.0),
        order.fee_amount,
        order_flags(order, signature),
        executed_amount,
        Bytes::from(signature.encode_for_settlement(owner)),
    )
}

fn order_flags(order: &OrderData, signature: &Signature) -> U256 {
    let mut result = 0u8;
    // The kind is encoded as 1 bit in position 0.
    result |= match order.kind {
        OrderKind::Sell => 0b0,
        OrderKind::Buy => 0b1,
    };
    // The order fill kind is encoded as 1 bit in position 1.
    result |= (order.partially_fillable as u8) << 1;
    // The order sell token balance is encoded as 2 bits in position 2.
    result |= match order.sell_token_balance {
        SellTokenSource::Erc20 => 0b00,
        SellTokenSource::External => 0b10,
        SellTokenSource::Internal => 0b11,
    } << 2;
    // The order buy token balance is encoded as 1 bit in position 4.
    result |= match order.buy_token_balance {
        BuyTokenDestination::Erc20 => 0b0,
        BuyTokenDestination::Internal => 0b1,
    } << 4;
    // The signing scheme is encoded as a 2 bits in position 5.
    result |= match signature.scheme() {
        SigningScheme::Eip712 => 0b00,
        SigningScheme::EthSign => 0b01,
        SigningScheme::Eip1271 => 0b10,
        SigningScheme::PreSign => 0b11,
    } << 5;
    U256::from(result)
}

/// An HTTP client factory.
///
/// This ensures a common configuration for all our HTTP clients used in various
/// places, while allowing for separate configurations, connection pools, and
/// cookie stores (for things like sessions and default headers) across
/// different APIs.
#[derive(Clone, Debug)]
pub struct HttpClientFactory {
    timeout: Duration,
}

impl HttpClientFactory {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Creates a new HTTP client with the default settings.
    pub fn create(&self) -> Client {
        self.builder().build().unwrap()
    }

    /// Creates a new HTTP client, allowing for additional configuration.
    pub fn configure(&self, config: impl FnOnce(ClientBuilder) -> ClientBuilder) -> Client {
        config(self.builder()).build().unwrap()
    }

    /// Returns a `ClientBuilder` with the default settings.
    pub fn builder(&self) -> ClientBuilder {
        const USER_AGENT: &str = "cowprotocol-services/2.0.0";
        ClientBuilder::new()
            .timeout(self.timeout)
            .tcp_keepalive(Duration::from_secs(60))
            .user_agent(USER_AGENT)
    }
}

impl Default for HttpClientFactory {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
        }
    }
}

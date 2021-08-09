// This application observes the order book api and tries to determine if the solver is down. It
// does this by checking if no trades have been made recently and if so checking if it finds a
// matchable order according to an external price api (matcha). If this is the case it alerts.

use anyhow::{Context, Result};
use model::{
    order::{Order, OrderCreation, OrderKind, OrderStatus, OrderUid, BUY_ETH_ADDRESS},
    u256_decimal,
};
use primitive_types::{H160, U256};
use reqwest::Client;
use std::time::{Duration, Instant, SystemTime};
use structopt::StructOpt;
use url::Url;

struct OrderBookApi {
    base: Url,
    client: Client,
}

impl OrderBookApi {
    pub fn new(client: Client, base_url: &str) -> Self {
        Self {
            base: base_url.parse().unwrap(),
            client,
        }
    }

    pub async fn solvable_orders(&self) -> reqwest::Result<Vec<Order>> {
        let mut url = self.base.clone();
        url.set_path("/api/v1/solvable_orders");
        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    pub async fn order(&self, uid: &OrderUid) -> reqwest::Result<Order> {
        let mut url = self.base.clone();
        url.set_path(&format!("/api/v1/orders/{}", uid));
        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}

// Converts the eth placeholder address to weth. Leaves other addresses untouched.
fn convert_eth_to_weth(token: H160) -> H160 {
    let weth: H160 = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        .parse()
        .unwrap();
    if token == BUY_ETH_ADDRESS {
        weth
    } else {
        token
    }
}

struct MatchaApi {
    base: Url,
    client: Client,
}

impl MatchaApi {
    pub fn new(client: Client) -> Self {
        Self {
            base: "https://api.0x.org".parse().unwrap(),
            client,
        }
    }

    pub async fn can_be_settled(&self, order: &OrderCreation) -> Result<bool> {
        let mut url = self.base.clone();
        url.set_path("/swap/v1/price");
        let (amount_name, amount) = match order.kind {
            OrderKind::Buy => ("buyAmount", order.buy_amount),
            OrderKind::Sell => ("sellAmount", order.sell_amount),
        };
        let buy_token = convert_eth_to_weth(order.buy_token);
        url.query_pairs_mut()
            .append_pair("sellToken", &format!("{:#x}", order.sell_token))
            .append_pair("buyToken", &format!("{:#x}", buy_token))
            .append_pair(amount_name, &amount.to_string());

        #[derive(Debug, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Response {
            #[serde(with = "u256_decimal")]
            pub sell_amount: U256,
            #[serde(with = "u256_decimal")]
            pub buy_amount: U256,
        }

        let response: Response = self
            .client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        tracing::debug!(url = url.as_str(), ?response, "matcha");

        Ok(match order.kind {
            OrderKind::Buy => order.sell_amount >= response.sell_amount,
            OrderKind::Sell => order.buy_amount <= response.buy_amount,
        })
    }
}

struct Alerter {
    orderbook_api: OrderBookApi,
    matcha_api: MatchaApi,
    config: AlertConfig,
    last_observed_trade: Instant,
    last_alert: Option<Instant>,
    open_orders: Vec<Order>,
}

struct AlertConfig {
    // Alert if no trades have been observed for this long.
    time_without_trade: Duration,
    // Give the solver some time to settle an order after it has been created before we alert.
    min_order_age: Duration,
    // Do not alert more often than this.
    min_alert_interval: Duration,
}

impl Alerter {
    pub fn new(orderbook_api: OrderBookApi, matcha_api: MatchaApi, config: AlertConfig) -> Self {
        Self {
            orderbook_api,
            matcha_api,
            config,
            last_observed_trade: Instant::now(),
            last_alert: None,
            open_orders: Vec::new(),
        }
    }

    async fn update_open_orders(&mut self) -> Result<()> {
        let mut orders = self
            .orderbook_api
            .solvable_orders()
            .await
            .context("solvable_orders")?;
        tracing::debug!("found {} open orders", orders.len());
        std::mem::swap(&mut self.open_orders, &mut orders);
        // Keep only orders that were open last update and are not open this update.
        orders.retain(|order| !self.open_orders.contains(order));
        for closed_order in orders {
            let order = self
                .orderbook_api
                .order(&closed_order.order_meta_data.uid)
                .await?;
            if order.order_meta_data.status == OrderStatus::Fulfilled {
                tracing::debug!(
                    "updating last observed trade because order {} was fulfilled",
                    order.order_meta_data.uid
                );
                self.last_observed_trade = Instant::now();
                break;
            }
        }
        tracing::debug!("found no fulfilled orders");
        Ok(())
    }

    fn order_has_minimum_age(&self, order: &Order) -> bool {
        let order_time =
            Duration::from_secs(order.order_meta_data.creation_date.timestamp() as u64);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        now.saturating_sub(order_time) > self.config.min_order_age
    }

    fn alert(&self, order: &Order) {
        tracing::error!(
            "No orders have been settled in the last {} seconds even though order {} is solvable and has a price that allows it to be settled according to matcha.",
            self.config.time_without_trade.as_secs(),
            order.order_meta_data.uid,
        );
    }

    pub async fn update(&mut self) -> Result<()> {
        self.update_open_orders().await?;
        if self.last_observed_trade.elapsed() <= self.config.time_without_trade {
            return Ok(());
        }
        if matches!(
            self.last_alert,
            Some(instant) if instant.elapsed() < self.config.min_alert_interval
        ) {
            return Ok(());
        }
        for order in &self.open_orders {
            if !self.order_has_minimum_age(order) {
                continue;
            }

            if self
                .matcha_api
                .can_be_settled(&order.order_creation)
                .await
                .context("can_be_settled")?
            {
                self.last_alert = Some(Instant::now());
                self.alert(order);
                break;
            }
        }
        Ok(())
    }
}

#[derive(Debug, StructOpt)]
struct Arguments {
    /// Minimum time without a trade before alerting.
    #[structopt(
        long,
        default_value = "600",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    time_without_trade: Duration,

    /// Minimum age a matchable order must have before alerting.
    #[structopt(
        long,
        default_value = "180",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    min_order_age: Duration,

    /// Do not repeat the alert more often than this.
    #[structopt(
        long,
        default_value = "1800",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    min_alert_interval: Duration,

    /// How many errors in the update loop (fetching solvable orders or querying matcha) in a row
    /// must happen before we alert about them.
    #[structopt(long, default_value = "5")]
    errors_in_a_row_before_alert: u32,

    #[structopt(long, default_value = "https://protocol-mainnet.gnosis.io")]
    orderbook_api: String,
}

#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    shared::tracing::initialize("alerter=debug");
    tracing::info!("running alerter with {:#?}", args);

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let mut alerter = Alerter::new(
        OrderBookApi::new(client.clone(), &args.orderbook_api),
        MatchaApi::new(client),
        AlertConfig {
            time_without_trade: args.time_without_trade,
            min_order_age: args.min_order_age,
            min_alert_interval: args.min_alert_interval,
        },
    );

    let mut errors_in_a_row = 0;
    loop {
        match alerter.update().await {
            Ok(()) => errors_in_a_row = 0,
            Err(err) if errors_in_a_row < args.errors_in_a_row_before_alert => {
                errors_in_a_row += 1;
                tracing::warn!(?err, "alerter update error");
            }
            Err(err) => {
                errors_in_a_row = 0;
                tracing::error!(?err, "alerter update error");
            }
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

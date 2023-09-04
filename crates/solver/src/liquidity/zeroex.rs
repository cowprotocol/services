use {
    super::{LimitOrderExecution, LimitOrderId, LiquidityOrderId, SettlementHandling},
    crate::{
        interactions::{
            allowances::{AllowanceManager, AllowanceManaging, Allowances},
            ZeroExInteraction,
        },
        liquidity::{Exchange, LimitOrder, Liquidity},
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    anyhow::{anyhow, Context, Result},
    contracts::{GPv2Settlement, IZeroEx},
    ethcontract::Address,
    futures::{SinkExt, StreamExt},
    model::{order::OrderKind, TokenPair},
    primitive_types::{H160, U256},
    reqwest::{IntoUrl, Url},
    shared::{
        ethrpc::Web3,
        http_solver::model::TokenAmount,
        recent_block_cache::Block,
        zeroex_api::{
            websocket::{OrderRecord, OrderState, OrdersResponse},
            Order,
            OrdersQuery,
            ZeroExApi,
        },
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
        time::Duration,
    },
    tokio::{net::TcpStream, sync::Mutex},
    tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream},
    tracing::Instrument,
};

const DEFAULT_ZEROEX_WEBSOCKET_API: &str = "wss://api.0x.org/orderbook/v1";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Clone)]
pub struct ZeroExWebsocketApi {
    url: Url,
}

impl ZeroExWebsocketApi {
    pub fn new(url: impl IntoUrl) -> Self {
        Self {
            url: url
                .into_url()
                .unwrap_or(Url::parse(DEFAULT_ZEROEX_WEBSOCKET_API).unwrap()),
        }
    }
}

impl ZeroExWebsocketApi {
    /// Creates a websocket connection to 0x and subscribes to orderbook
    /// updates. Once this function is done, the websocket is open and ready
    /// to receive messages.
    async fn open_socket(&self) -> Result<Socket> {
        let mut socket = tokio_tungstenite::connect_async(self.url.clone()).await?.0;

        // Construct the subscription message to fetch all orders
        let subscription_msg = serde_json::json!({
            "type": "subscribe",
            "channel": "orders",
            "requestId": "cowswap",
        });

        socket
            .send(Message::Text(subscription_msg.to_string()))
            .await?;

        Ok(socket)
    }
}

#[derive(Debug)]
enum UpdateError {
    // The websocket is closed on server side, with optional reason
    SocketClosed(Option<anyhow::Error>),
    // Received a message type that we don't handle
    UnsupportedMessage,
    // Received proper `text` type of message but it can't be deserialized (mailformed or changed
    // format)
    DeserializeError(anyhow::Error),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for UpdateError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<UpdateError> for anyhow::Error {
    fn from(err: UpdateError) -> Self {
        match err {
            UpdateError::SocketClosed(err) => anyhow::anyhow!("socket closed, reason {:?}", err),
            UpdateError::UnsupportedMessage => anyhow::anyhow!("unsupported message"),
            UpdateError::DeserializeError(err) => err,
            UpdateError::Other(err) => err,
        }
    }
}

#[derive(Clone)]
struct ZeroExCache {
    /// Fetch orders that can be filled by this address
    sender: Address,
    orders: Arc<Mutex<HashMap<OrderId, OrderRecord>>>,
}

impl ZeroExCache {
    pub fn new(sender: Address) -> Self {
        Self {
            sender,
            orders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Calls the 0x API to get all orders and initializes the cache with them.
    async fn init_cache(&self, api: Arc<dyn ZeroExApi>) -> Result<()> {
        {
            // init can be called multiple times, so clear the cache first
            // clearing is done before fetching 0x orders because fetching can keep failing
            // for a long time and we don't want to keep stale orders
            let mut cache = self.orders.lock().await;
            cache.clear();
        }

        let queries = &[
            // orders fillable by anyone
            OrdersQuery::default(),
            // orders fillable only by our settlement contract
            OrdersQuery {
                sender: Some(self.sender),
                ..Default::default()
            },
        ];

        let zeroex_orders =
            futures::future::try_join_all(queries.iter().map(|query| api.get_orders(query)))
                .await
                .context("failed to fetch 0x limit orders")?;
        let zeroex_orders: Vec<_> = zeroex_orders.into_iter().flatten().collect();

        let mut cache = self.orders.lock().await;
        cache.extend(
            zeroex_orders
                .into_iter()
                .map(|order| (hex::encode(&order.metadata.order_hash), order.into())),
        );
        tracing::debug!("Initialized 0x cache with {} orders", cache.len());

        Ok(())
    }

    /// Creates a websocket connection and reads the messages from it.
    async fn connect_and_update_cache(&self, api: ZeroExWebsocketApi) -> Result<(), UpdateError> {
        let mut socket = api.open_socket().await?;
        let result = self.update_cache(&mut socket).await;
        // this call will error if the socket is already closed but that's fine
        let _ = socket.close(None).await;
        result
    }

    /// Indifinitelly reads messages from the websocket and updates the cache.
    async fn update_cache(&self, socket: &mut Socket) -> Result<(), UpdateError> {
        while let Some(msg) = socket.next().await {
            let msg = msg.context("websocket error")?;
            let text = match msg {
                Message::Text(text) => text,
                Message::Close(frame) => {
                    return Err(UpdateError::SocketClosed(
                        frame.map(|frame| anyhow!(frame.reason)),
                    ))
                }
                Message::Ping(payload) => {
                    // send pong message to keep the connection alive
                    socket
                        .send(Message::Pong(payload))
                        .await
                        .context("ping pong failure")?;
                    continue;
                }
                _ => {
                    tracing::error!("Received unsupported message {:?}", msg);
                    return Err(UpdateError::UnsupportedMessage);
                }
            };

            let records = serde_json::from_str::<OrdersResponse>(&text)
                .with_context(|| format!("deserialization error, text received: {}", text))
                .map_err(UpdateError::DeserializeError)?
                .payload;

            let mut cache = self.orders.lock().await;
            for record in records {
                match record.metadata.state {
                    OrderState::Added | OrderState::Updated | OrderState::Fillable => {
                        cache.insert(hex::encode(record.metadata.order_hash.clone()), record);
                    }
                    OrderState::Expired => {
                        cache.remove(&hex::encode(record.metadata.order_hash));
                    }
                }
            }
        }

        Ok(())
    }

    /// Main update loop
    pub async fn update(&self, init_api: Arc<dyn ZeroExApi>, update_api: ZeroExWebsocketApi) {
        let mut backoff = DEFAULT_BACKOFF;
        // loop needed for reconnections
        loop {
            // initialize cache by fetching all existing orders via http api
            if let Err(err) = self.init_cache(init_api.clone()).await {
                tracing::warn!(
                    "Error initializing 0x cache: {:?}, retrying with backoff {:?} ...",
                    err,
                    backoff
                );
                tokio::time::sleep(backoff).await;
                backoff *= 2;
                continue;
            }

            backoff = DEFAULT_BACKOFF;
            // from now on, rely on websocket connection to do incremental updates
            if let Err(err) = self.connect_and_update_cache(update_api.clone()).await {
                tracing::debug!("Error updating 0x cache, reconnecting... {:?}", err);
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
        }
    }

    pub async fn orders(&self) -> Vec<OrderRecord> {
        self.orders
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>()
    }
}

pub struct ZeroExLiquidity {
    pub zeroex: IZeroEx,
    pub gpv2: GPv2Settlement,
    pub allowance_manager: Box<dyn AllowanceManaging>,
    cache: ZeroExCache,
}

// hash generated by the 0x api
type OrderId = String;
type OrderBuckets = HashMap<(H160, H160), Vec<OrderRecord>>;
const DEFAULT_BACKOFF: Duration = Duration::from_secs(3);

impl ZeroExLiquidity {
    pub fn new(
        web3: Web3,
        init_api: Arc<dyn ZeroExApi>,
        zeroex: IZeroEx,
        gpv2: GPv2Settlement,
        ws_url: impl IntoUrl,
    ) -> Self {
        let allowance_manager = AllowanceManager::new(web3, gpv2.address());
        let cache = ZeroExCache::new(gpv2.address());

        {
            // spawn background task to continually update the cache
            let inner = cache.clone();
            let update_api = ZeroExWebsocketApi::new(ws_url);
            tokio::task::spawn(
                async move {
                    inner.update(init_api, update_api).await;
                }
                .instrument(tracing::debug_span!("0x cache updater")),
            );
        }

        Self {
            zeroex,
            gpv2,
            allowance_manager: Box::new(allowance_manager),
            cache,
        }
    }

    /// Turns 0x OrderRecord into liquidity which solvers can use.
    fn record_into_liquidity(
        &self,
        record: OrderRecord,
        allowances: Arc<Allowances>,
    ) -> Option<Liquidity> {
        let sell_amount: U256 = record.remaining_maker_amount().ok()?.into();
        if sell_amount.is_zero() || record.metadata.remaining_fillable_taker_amount == 0 {
            // filter out orders with 0 amounts to prevent errors in the solver
            return None;
        }

        let limit_order = LimitOrder {
            id: LimitOrderId::Liquidity(LiquidityOrderId::ZeroEx(hex::encode(
                &record.metadata.order_hash,
            ))),
            sell_token: record.order.maker_token,
            buy_token: record.order.taker_token,
            sell_amount,
            buy_amount: record.metadata.remaining_fillable_taker_amount.into(),
            kind: OrderKind::Buy,
            partially_fillable: true,
            solver_fee: U256::zero(),
            settlement_handling: Arc::new(OrderSettlementHandler {
                order: record.order,
                zeroex: self.zeroex.clone(),
                allowances,
            }),
            exchange: Exchange::ZeroEx,
        };
        Some(Liquidity::LimitOrder(limit_order))
    }
}

#[async_trait::async_trait]
impl LiquidityCollecting for ZeroExLiquidity {
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        _block: Block,
    ) -> Result<Vec<Liquidity>> {
        let zeroex_orders = self.cache.orders().await;
        tracing::debug!("Fetched {} orders from 0x", zeroex_orders.len());

        let order_buckets = generate_order_buckets(zeroex_orders.into_iter(), pairs);
        let filtered_zeroex_orders = get_useful_orders(order_buckets, 5);
        let tokens: HashSet<_> = filtered_zeroex_orders
            .iter()
            .map(|o| o.order.taker_token)
            .collect();

        let allowances = Arc::new(
            self.allowance_manager
                .get_allowances(tokens, self.zeroex.address())
                .await?,
        );

        let zeroex_liquidity_orders: Vec<_> = filtered_zeroex_orders
            .into_iter()
            .flat_map(|order| self.record_into_liquidity(order, allowances.clone()))
            .collect();

        Ok(zeroex_liquidity_orders)
    }
}

fn generate_order_buckets(
    zeroex_orders: impl Iterator<Item = OrderRecord>,
    relevant_pairs: HashSet<TokenPair>,
) -> OrderBuckets {
    // divide orders in buckets
    let mut buckets = OrderBuckets::default();
    zeroex_orders
        .filter(
            |record| match TokenPair::new(record.order.taker_token, record.order.maker_token) {
                Some(pair) => relevant_pairs.contains(&pair),
                None => false,
            },
        )
        .for_each(|order| {
            let bucket = buckets
                .entry((order.order.taker_token, order.order.maker_token))
                .or_default();
            bucket.push(order);
        });
    buckets
}

/// Get the `orders_per_type` best priced and biggest volume orders.
fn get_useful_orders(order_buckets: OrderBuckets, orders_per_type: usize) -> Vec<OrderRecord> {
    let mut filtered_zeroex_orders = vec![];
    for mut orders in order_buckets.into_values() {
        if orders.len() <= 2 * orders_per_type {
            filtered_zeroex_orders.extend(orders);
            continue;
        }
        // Sorting to have best priced orders at the end of the vector
        // best priced orders are those that have the maximum maker_amount /
        // taker_amount ratio
        orders.sort_by(|order_1, order_2| {
            let price_1 = order_1.order.maker_amount as f64 / order_1.order.taker_amount as f64;
            let price_2 = order_2.order.maker_amount as f64 / order_2.order.taker_amount as f64;
            price_1.total_cmp(&price_2)
        });
        filtered_zeroex_orders.extend(orders.drain(orders.len() - orders_per_type..));

        orders.sort_by_key(|order| order.metadata.remaining_fillable_taker_amount);
        filtered_zeroex_orders.extend(orders.into_iter().rev().take(orders_per_type));
    }
    filtered_zeroex_orders
}

struct OrderSettlementHandler {
    order: Order,
    zeroex: IZeroEx,
    allowances: Arc<Allowances>,
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(
        &self,
        execution: LimitOrderExecution,
        encoder: &mut SettlementEncoder,
    ) -> Result<()> {
        if execution.filled > u128::MAX.into() {
            anyhow::bail!("0x only supports executed amounts of size u128");
        }
        let approval = self
            .allowances
            .approve_token(TokenAmount::new(self.order.taker_token, execution.filled))?;
        if let Some(approval) = approval {
            encoder.append_to_execution_plan(Arc::new(approval));
        }
        encoder.append_to_execution_plan(Arc::new(ZeroExInteraction {
            taker_token_fill_amount: execution.filled.as_u128(),
            order: self.order.clone(),
            zeroex: self.zeroex.clone(),
        }));
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        crate::interactions::allowances::Approval,
        maplit::hashmap,
        shared::{
            baseline_solver::BaseTokens,
            ethrpc::create_env_test_transport,
            http_solver::model::InternalizationStrategy,
            interaction::Interaction,
            zeroex_api::{websocket::OrderMetadata, DefaultZeroExApi},
        },
        std::time::Duration,
    };

    fn get_relevant_pairs(token_a: H160, token_b: H160) -> HashSet<TokenPair> {
        let base_tokens = Arc::new(BaseTokens::new(H160::zero(), &[]));
        let fake_order = [TokenPair::new(token_a, token_b).unwrap()].into_iter();
        base_tokens.relevant_pairs(fake_order)
    }

    #[test]
    fn order_buckets_get_created() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order = Order::default();
        let metadata = OrderMetadata::default();
        let order_with_tokens = |token_a, token_b| OrderRecord {
            order: Order {
                taker_token: token_a,
                maker_token: token_b,
                ..order.clone()
            },
            metadata: metadata.clone(),
        };
        let order_1 = order_with_tokens(token_a, token_b);
        let order_2 = order_with_tokens(token_b, token_a);
        let order_3 = order_with_tokens(token_b, token_a);
        let order_buckets =
            generate_order_buckets([order_1, order_2, order_3].into_iter(), relevant_pairs);
        assert_eq!(order_buckets.keys().len(), 2);
        assert_eq!(order_buckets[&(token_a, token_b)].len(), 1);
        assert_eq!(order_buckets[&(token_b, token_a)].len(), 2);
    }

    #[test]
    fn empty_bucket_no_relevant_orders() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let token_ignore = H160([0x11; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order = Order::default();
        let metadata = OrderMetadata::default();
        let order_with_tokens = |token_a, token_b| OrderRecord {
            order: Order {
                taker_token: token_a,
                maker_token: token_b,
                ..order.clone()
            },
            metadata: metadata.clone(),
        };
        let order_1 = order_with_tokens(token_ignore, token_b);
        let order_2 = order_with_tokens(token_a, token_ignore);
        let order_3 = order_with_tokens(token_ignore, token_ignore);
        let order_buckets =
            generate_order_buckets([order_1, order_2, order_3].into_iter(), relevant_pairs);
        let filtered_zeroex_orders = get_useful_orders(order_buckets, 1);
        assert_eq!(filtered_zeroex_orders.len(), 0);
    }

    #[test]
    fn biggest_volume_orders_get_selected() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_with_fillable_amount = |remaining_fillable_taker_amount| OrderRecord {
            order: Order {
                taker_token: token_a,
                maker_token: token_b,
                taker_amount: 100_000_000,
                maker_amount: 100_000_000,
                ..Default::default()
            },
            metadata: OrderMetadata {
                remaining_fillable_taker_amount,
                ..Default::default()
            },
        };
        let order_1 = order_with_fillable_amount(1_000);
        let order_2 = order_with_fillable_amount(100);
        let order_3 = order_with_fillable_amount(10_000);
        let order_buckets =
            generate_order_buckets([order_1, order_2, order_3].into_iter(), relevant_pairs);
        let filtered_zeroex_orders = get_useful_orders(order_buckets, 1);
        assert_eq!(filtered_zeroex_orders.len(), 2);
        assert_eq!(
            filtered_zeroex_orders[0]
                .metadata
                .remaining_fillable_taker_amount,
            10_000
        );
        assert_eq!(
            filtered_zeroex_orders[1]
                .metadata
                .remaining_fillable_taker_amount,
            1_000
        );
    }

    #[test]
    fn best_priced_orders_get_selected() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_with_amount = |taker_amount, remaining_fillable_taker_amount| OrderRecord {
            order: Order {
                taker_token: token_a,
                maker_token: token_b,
                taker_amount,
                maker_amount: 100_000_000,
                ..Default::default()
            },
            metadata: OrderMetadata {
                remaining_fillable_taker_amount,
                ..Default::default()
            },
        };
        let order_1 = order_with_amount(10_000_000, 1_000_000);
        let order_2 = order_with_amount(1_000, 100);
        let order_3 = order_with_amount(100_000, 1_000);
        let order_buckets =
            generate_order_buckets([order_1, order_2, order_3].into_iter(), relevant_pairs);
        let filtered_zeroex_orders = get_useful_orders(order_buckets, 1);
        assert_eq!(filtered_zeroex_orders.len(), 2);
        // First item in the list will be on the basis of maker_amount/taker_amount
        // ratio
        assert_eq!(filtered_zeroex_orders[0].order.taker_amount, 1_000);
        // Second item in the list will be on the basis of
        // remaining_fillable_taker_amount
        assert_eq!(filtered_zeroex_orders[1].order.taker_amount, 10_000_000);
    }

    #[tokio::test]
    async fn interaction_encodes_approval_when_insufficient() {
        let sell_token = H160::from_low_u64_be(1);
        let zeroex = shared::dummy_contract!(IZeroEx, H160::default());
        let allowances = Allowances::new(zeroex.address(), hashmap! { sell_token => 99.into() });
        let order = Order {
            taker_amount: 100,
            taker_token: sell_token,
            ..Default::default()
        };
        let handler = OrderSettlementHandler {
            order: order.clone(),
            zeroex: zeroex.clone(),
            allowances: Arc::new(allowances),
        };
        let mut encoder = SettlementEncoder::default();
        let execution = LimitOrderExecution::new(100.into(), 0.into());
        handler.encode(execution, &mut encoder).unwrap();
        let [_, interactions, _] = encoder
            .finish(InternalizationStrategy::SkipInternalizableInteraction)
            .interactions;
        assert_eq!(
            interactions,
            [
                Approval {
                    token: sell_token,
                    spender: zeroex.address(),
                }
                .encode(),
                ZeroExInteraction {
                    order,
                    taker_token_fill_amount: 100,
                    zeroex
                }
                .encode(),
            ]
            .concat(),
        );
    }

    #[tokio::test]
    async fn interaction_encodes_no_approval_when_sufficient() {
        let sell_token = H160::from_low_u64_be(1);
        let zeroex = shared::dummy_contract!(IZeroEx, H160::default());
        let allowances = Allowances::new(zeroex.address(), hashmap! { sell_token => 100.into() });
        let order = Order {
            taker_amount: 100,
            taker_token: sell_token,
            ..Default::default()
        };
        let handler = OrderSettlementHandler {
            order: order.clone(),
            zeroex: zeroex.clone(),
            allowances: Arc::new(allowances),
        };
        let mut encoder = SettlementEncoder::default();
        let execution = LimitOrderExecution::new(100.into(), 0.into());
        handler.encode(execution, &mut encoder).unwrap();
        let [_, interactions, _] = encoder
            .finish(InternalizationStrategy::SkipInternalizableInteraction)
            .interactions;
        assert_eq!(
            interactions,
            [ZeroExInteraction {
                order,
                taker_token_fill_amount: 100,
                zeroex
            }
            .encode(),]
            .concat(),
        );
    }

    #[tokio::test]
    #[ignore]
    async fn connect_and_update_cache_test() {
        let sender = {
            let transport = create_env_test_transport();
            let web3 = Web3::new(transport);
            GPv2Settlement::deployed(&web3).await.unwrap().address()
        };
        let cache = ZeroExCache::new(sender);
        let init_api = Arc::new(DefaultZeroExApi::test());
        let update_api = ZeroExWebsocketApi::new(DEFAULT_ZEROEX_WEBSOCKET_API);

        let inner = cache.clone();
        tokio::task::spawn(async move {
            inner.update(init_api, update_api).await;
        });

        // read cache from outside
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;

            let cache_size = cache.orders().await.len();
            println!("reader size: {}", cache_size);
        }
    }
}

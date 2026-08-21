use {
    crate::boundary,
    alloy::primitives::{Address, U256},
    configs::autopilot::penalty_cap::{PenaltyCapConfig, PenaltyFactor},
    eth_domain_types as eth,
    model::order::{BUY_ETH_ADDRESS, OrderKind},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Mutex,
    },
};

/// 10^18, the scaling factor of normalized native prices.
const WAD: U256 = U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);

/// Computes per-order penalty caps (CIP-87): the maximum penalty a solver
/// can incur for winning an order but failing to execute it, expressed in
/// the native token.
///
/// The cap is a fraction of the order's volume, bounded by a configured
/// USD amount. The fraction depends on whether the traded tokens belong to
/// a configured bucket (e.g. correlated tokens).
pub struct PenaltyCapCalculator {
    default_factor: PenaltyFactor,
    overrides: Vec<Override>,
    /// The USD bound converted into atoms of the reference token.
    absolute_cap_atoms: U256,
    usd_reference_token: Address,
    native_token: Address,
    /// Most recently fetched native price of the USD reference token,
    /// normalized like auction prices (native token wei per 10^18 atoms).
    usd_reference_price: Mutex<U256>,
}

struct Override {
    tokens: HashSet<Address>,
    factor: PenaltyFactor,
}

impl PenaltyCapCalculator {
    pub fn new(
        config: &PenaltyCapConfig,
        native_token: Address,
        usd_reference_token_decimals: u8,
        initial_usd_reference_price: U256,
    ) -> Self {
        Metrics::get()
            .usd_price_last_update
            .set(chrono::Utc::now().timestamp());
        Self {
            default_factor: config.default_factor,
            overrides: config
                .overrides
                .iter()
                .map(|override_| Override {
                    tokens: override_.tokens.clone(),
                    factor: override_.factor,
                })
                .collect(),
            absolute_cap_atoms: U256::from(
                (config.absolute_cap_usd * 10f64.powi(i32::from(usd_reference_token_decimals)))
                    .round() as u128,
            ),
            usd_reference_token: config.usd_reference_token,
            native_token,
            usd_reference_price: Mutex::new(initial_usd_reference_price),
        }
    }

    pub fn usd_reference_token(&self) -> Address {
        self.usd_reference_token
    }

    /// Records the latest native price of the USD reference token. Staleness
    /// (i.e. this not being called) is monitored via the last update metric.
    pub fn record_usd_price(&self, price: U256) {
        *self.usd_reference_price.lock().unwrap() = price;
        Metrics::get()
            .usd_price_last_update
            .set(chrono::Utc::now().timestamp());
    }

    /// Computes the penalty cap for an order, in native token.
    ///
    /// The order's volume is its buy amount for sell orders and its sell
    /// amount for buy orders, converted using the auction's native prices.
    /// If the volume cannot be determined (missing native price or
    /// overflow) the absolute USD bound applies.
    pub fn calculate(
        &self,
        order: &boundary::Order,
        prices: &BTreeMap<Address, U256>,
    ) -> eth::Ether {
        let (token, amount) = match order.data.kind {
            OrderKind::Sell => (order.data.buy_token, order.data.buy_amount),
            OrderKind::Buy => (order.data.sell_token, order.data.sell_amount),
        };
        let absolute_cap = self.absolute_cap_in_native();
        let factor = self.factor(order.data.sell_token, order.data.buy_token);
        let volume_cap = prices.get(&self.wrapped(token)).and_then(|price| {
            amount
                .checked_mul(*price)
                .map(|volume| volume / WAD)
                .and_then(|volume| factor.apply_to(volume))
        });
        let cap = match volume_cap {
            Some(volume_cap) => volume_cap.min(absolute_cap),
            None => absolute_cap,
        };
        eth::Ether(cap)
    }

    /// Determines the applicable volume factor for a token pair.
    fn factor(&self, sell_token: Address, buy_token: Address) -> PenaltyFactor {
        let buy_token = self.wrapped(buy_token);
        self.overrides
            .iter()
            .find(|override_| {
                override_.tokens.contains(&sell_token) && override_.tokens.contains(&buy_token)
            })
            .map(|override_| override_.factor)
            .unwrap_or(self.default_factor)
    }

    /// Treats native ETH like the wrapped native token, so that buckets
    /// and the price map don't need to contain the ETH marker address.
    fn wrapped(&self, token: Address) -> Address {
        if token == BUY_ETH_ADDRESS {
            self.native_token
        } else {
            token
        }
    }

    /// The absolute USD bound converted into native token using the last
    /// known price of the reference token.
    fn absolute_cap_in_native(&self) -> U256 {
        let price = *self.usd_reference_price.lock().unwrap();
        self.absolute_cap_atoms
            .checked_mul(price)
            .map(|cap| cap / WAD)
            .unwrap_or(U256::MAX)
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "penalty_cap")]
struct Metrics {
    /// Unix timestamp (in seconds) of the last successful update of the
    /// USD reference token's native price.
    usd_price_last_update: prometheus::IntGauge,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        configs::autopilot::penalty_cap::PenaltyCapOverride,
        model::order::{Order, OrderData},
    };

    const NATIVE: Address = Address::repeat_byte(0xff);
    const USDC: Address = Address::repeat_byte(0x01);
    const WSTETH: Address = Address::repeat_byte(0x02);
    const COW: Address = Address::repeat_byte(0x03);

    fn calculator() -> PenaltyCapCalculator {
        PenaltyCapCalculator::new(
            &PenaltyCapConfig {
                default_factor: 0.0004.try_into().unwrap(),
                absolute_cap_usd: 20.,
                usd_reference_token: USDC,
                overrides: vec![PenaltyCapOverride {
                    tokens: [NATIVE, WSTETH].into_iter().collect(),
                    factor: 0.00001.try_into().unwrap(),
                }],
            },
            NATIVE,
            6,
            // 1 USDC atom is worth 2.5e8 wei (i.e. ETH at $4000), so the
            // $20 bound equals 5e15 wei.
            U256::from(250_000_000_000_000_000_000_000_000_u128),
        )
    }

    fn order(kind: OrderKind, sell: (Address, u128), buy: (Address, u128)) -> Order {
        Order {
            data: OrderData {
                sell_token: sell.0,
                sell_amount: U256::from(sell.1),
                buy_token: buy.0,
                buy_amount: U256::from(buy.1),
                kind,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn sell_order_uses_buy_amount() {
        // 1 native token of buy volume capped at 4 bps. The sell token has
        // no native price, which must not matter for sell orders.
        let order = order(
            OrderKind::Sell,
            (USDC, 4_000_000_000),
            (COW, 1_000_000_000_000_000_000),
        );
        // 1e18 COW atoms are worth 1 native token.
        let prices = BTreeMap::from([(COW, U256::from(1_000_000_000_000_000_000_u128))]);
        let cap = calculator().calculate(&order, &prices);
        assert_eq!(cap.0, U256::from(400_000_000_000_000_u128));
    }

    #[test]
    fn buy_order_uses_sell_amount() {
        // 2 native tokens of sell volume capped at 4 bps.
        let order = order(
            OrderKind::Buy,
            (COW, 2_000_000_000_000_000_000),
            (USDC, 8_000_000_000),
        );
        // 1e18 COW atoms are worth 1 native token.
        let prices = BTreeMap::from([(COW, U256::from(1_000_000_000_000_000_000_u128))]);
        let cap = calculator().calculate(&order, &prices);
        assert_eq!(cap.0, U256::from(800_000_000_000_000_u128));
    }

    #[test]
    fn applies_bucket_override() {
        // Both tokens in the override bucket: 0.1 bps applies.
        let order = order(
            OrderKind::Buy,
            (WSTETH, 1_000_000_000_000_000_000),
            (NATIVE, 1_000_000_000_000_000_000),
        );
        // 1e18 WSTETH atoms are worth 1 native token.
        let prices = BTreeMap::from([(WSTETH, U256::from(1_000_000_000_000_000_000_u128))]);
        let cap = calculator().calculate(&order, &prices);
        assert_eq!(cap.0, U256::from(10_000_000_000_000_u128));
    }

    #[test]
    fn absolute_cap_binds_for_large_volumes() {
        // 100 native tokens of volume: 4 bps would be 4e16 wei, but the
        // $20 bound (5e15 wei) applies.
        let order = order(
            OrderKind::Sell,
            (USDC, 400_000_000_000),
            (COW, 100_000_000_000_000_000_000),
        );
        // 1e18 COW atoms are worth 1 native token.
        let prices = BTreeMap::from([(COW, U256::from(1_000_000_000_000_000_000_u128))]);
        let cap = calculator().calculate(&order, &prices);
        assert_eq!(cap.0, U256::from(5_000_000_000_000_000_u128));
    }

    #[test]
    fn overflowing_volume_falls_back_to_absolute_cap() {
        let order = Order {
            data: OrderData {
                sell_token: COW,
                sell_amount: U256::MAX,
                buy_token: USDC,
                buy_amount: U256::from(1_u128),
                kind: OrderKind::Buy,
                ..Default::default()
            },
            ..Default::default()
        };
        // 1e18 COW atoms are worth 1 native token.
        let prices = BTreeMap::from([(COW, U256::from(1_000_000_000_000_000_000_u128))]);
        let cap = calculator().calculate(&order, &prices);
        assert_eq!(cap.0, U256::from(5_000_000_000_000_000_u128));
    }

    #[test]
    fn missing_price_falls_back_to_absolute_cap() {
        // Buy orders need the sell token's native price, which is missing.
        let order = order(
            OrderKind::Buy,
            (USDC, 4_000_000_000),
            (COW, 1_000_000_000_000_000_000),
        );
        let cap = calculator().calculate(&order, &BTreeMap::default());
        assert_eq!(cap.0, U256::from(5_000_000_000_000_000_u128));
    }

    #[test]
    fn usd_price_update_changes_absolute_cap() {
        let calculator = calculator();
        // A large enough order for the absolute cap to apply.
        let order = order(
            OrderKind::Sell,
            (USDC, 400_000_000_000),
            (COW, 100_000_000_000_000_000_000),
        );
        // 1e18 COW atoms are worth 1 native token.
        let prices = BTreeMap::from([(COW, U256::from(1_000_000_000_000_000_000_u128))]);

        // Halving the reference token's price halves the absolute cap.
        calculator.record_usd_price(U256::from(125_000_000_000_000_000_000_000_000_u128));
        let cap = calculator.calculate(&order, &prices);
        assert_eq!(cap.0, U256::from(2_500_000_000_000_000_u128));
    }
}

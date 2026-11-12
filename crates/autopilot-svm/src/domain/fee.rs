//! Which protocol fee policies apply to an order.

use {
    crate::domain::auction::Order,
    chain_types::solana::{Pubkey, Solana},
    chrono::{DateTime, Utc},
    configs::autopilot::fee_policy::{
        FeePoliciesConfig,
        FeePolicy as PolicyConfig,
        FeePolicyKind,
        FeePolicyOrderClass,
    },
};

/// A protocol fee policy as the driver applies it to a fill.
pub type FeePolicy = protocol_fees::FeePolicy<Solana>;

/// The quote an order was placed against. The quote's execution cost is
/// netted into its amounts, so `fee` is zero.
pub type Quote = protocol_fees::Quote<Solana>;

/// The configured protocol fee policies, the EVM autopilot's `[fee-policies]`
/// section. A policy applies by the order's class: market when the linked
/// quote meets the order's limit, limit otherwise.
#[derive(Clone, Debug, Default)]
pub struct ProtocolFees {
    config: FeePoliciesConfig,
}

impl ProtocolFees {
    pub fn new(config: FeePoliciesConfig) -> Self {
        Self { config }
    }

    /// The policies that apply to `order` at `now`, in application order.
    pub fn apply(&self, order: &Order, now: DateTime<Utc>) -> Vec<FeePolicy> {
        // An order without a linked quote is out of market, through the same
        // zero-buy reference quote as on EVM.
        let quote = order.quote.unwrap_or(Quote {
            sell_amount: order.sell_amount,
            buy_amount: 0,
            fee: 0,
            solver: Pubkey([0; 32]),
        });
        let in_market = in_market(order, &quote);
        self.policies_at(now)
            .iter()
            .filter(|policy| match policy.order_class {
                FeePolicyOrderClass::Any => true,
                FeePolicyOrderClass::Market => in_market,
                FeePolicyOrderClass::Limit => !in_market,
            })
            .map(|policy| match &policy.kind {
                FeePolicyKind::Surplus {
                    factor,
                    max_volume_factor,
                } => FeePolicy::Surplus {
                    factor: factor.get(),
                    max_volume_factor: max_volume_factor.get(),
                },
                FeePolicyKind::PriceImprovement {
                    factor,
                    max_volume_factor,
                } => FeePolicy::PriceImprovement {
                    factor: factor.get(),
                    max_volume_factor: max_volume_factor.get(),
                    quote,
                },
                FeePolicyKind::Volume { factor } => FeePolicy::Volume {
                    factor: factor.get(),
                },
            })
            .collect()
    }

    /// The upcoming policies once their timestamp has passed, the current
    /// ones until then. An empty upcoming list never takes over, like on EVM.
    fn policies_at(&self, now: DateTime<Utc>) -> &[PolicyConfig] {
        let upcoming = &self.config.upcoming_policies;
        match upcoming.effective_from_timestamp {
            Some(effective) if effective <= now && !upcoming.policies.is_empty() => {
                &upcoming.policies
            }
            _ => &self.config.policies,
        }
    }
}

/// Whether the order's limit sits within its quote: the quote buys at least
/// the limit's rate. The EVM `is_order_outside_market_price` check with the
/// quote's fee at zero, exact in `u128`.
fn in_market(order: &Order, quote: &Quote) -> bool {
    u128::from(order.sell_amount) * u128::from(quote.buy_amount)
        >= u128::from(quote.sell_amount) * u128::from(order.buy_amount)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::auction::OrderKind,
        chain_types::solana::{AppData, IntentHash},
    };

    fn order(kind: OrderKind, quote: Option<Quote>) -> Order {
        Order {
            uid: IntentHash([1; 32]),
            owner: Pubkey([2; 32]),
            receiver: None,
            sell_token: Pubkey([3; 32]),
            buy_token: Pubkey([4; 32]),
            sell_token_account: Pubkey([5; 32]),
            buy_token_account: Pubkey([6; 32]),
            sell_amount: 1_000,
            buy_amount: 2_000,
            valid_to: 42,
            kind,
            partially_fillable: false,
            order_pda: Pubkey([7; 32]),
            app_data: AppData([0; 32]),
            created_on_chain: true,
            executed: 0,
            creation: None,
            sell_balance: None,
            quote,
            protocol_fees: Vec::new(),
        }
    }

    fn quote(sell_amount: u64, buy_amount: u64) -> Quote {
        Quote {
            sell_amount,
            buy_amount,
            fee: 0,
            solver: Pubkey([9; 32]),
        }
    }

    fn from_toml(toml: &str) -> ProtocolFees {
        ProtocolFees::new(toml::from_str(toml).unwrap())
    }

    const BY_CLASS: &str = r#"
        [[policies]]
        kind.surplus = { factor = 0.5, max-volume-factor = 0.9 }
        order-class = "limit"

        [[policies]]
        kind.price-improvement = { factor = 0.3, max-volume-factor = 0.01 }
        order-class = "market"

        [[policies]]
        kind.volume = { factor = 0.001 }
        order-class = "any"
    "#;

    /// Selling 1000 for 2000: a quote buying 2000 or more for 1000 puts the
    /// order in market, one buying less, or none, makes it a limit order.
    /// The policies keep the config order and a price improvement policy
    /// carries the linked quote.
    #[test]
    fn a_policy_applies_by_the_orders_class() {
        let fees = from_toml(BY_CLASS);
        let now = Utc::now();
        let market = fees.apply(&order(OrderKind::Sell, Some(quote(1_000, 2_000))), now);
        assert_eq!(
            market,
            vec![
                FeePolicy::PriceImprovement {
                    factor: 0.3,
                    max_volume_factor: 0.01,
                    quote: quote(1_000, 2_000),
                },
                FeePolicy::Volume { factor: 0.001 },
            ]
        );
        let limit = vec![
            FeePolicy::Surplus {
                factor: 0.5,
                max_volume_factor: 0.9,
            },
            FeePolicy::Volume { factor: 0.001 },
        ];
        assert_eq!(
            fees.apply(&order(OrderKind::Sell, Some(quote(1_000, 1_999))), now),
            limit
        );
        assert_eq!(fees.apply(&order(OrderKind::Sell, None), now), limit);
        // A buy order compares the same rate.
        assert!(matches!(
            fees.apply(&order(OrderKind::Buy, Some(quote(999, 2_000))), now)[..],
            [FeePolicy::PriceImprovement { quote: linked, .. }, FeePolicy::Volume { .. }]
                if linked == quote(999, 2_000)
        ));
        assert_eq!(
            fees.apply(&order(OrderKind::Buy, Some(quote(1_001, 2_000))), now),
            limit
        );
    }

    /// A price improvement policy on an order without a linked quote gets
    /// the zero-buy reference quote, like on EVM.
    #[test]
    fn an_unlinked_order_gets_the_zero_buy_reference_quote() {
        let fees = from_toml(
            r#"
            [[policies]]
            kind.price-improvement = { factor = 0.3, max-volume-factor = 0.01 }
            order-class = "any"
            "#,
        );
        assert_eq!(
            fees.apply(&order(OrderKind::Sell, None), Utc::now()),
            vec![FeePolicy::PriceImprovement {
                factor: 0.3,
                max_volume_factor: 0.01,
                quote: Quote {
                    sell_amount: 1_000,
                    buy_amount: 0,
                    fee: 0,
                    solver: Pubkey([0; 32]),
                },
            }]
        );
    }

    #[test]
    fn upcoming_policies_take_over_at_their_timestamp() {
        let fees = from_toml(
            r#"
            [[policies]]
            kind.volume = { factor = 0.001 }
            order-class = "any"

            [upcoming-policies]
            effective-from-timestamp = "2026-01-01T00:00:00Z"

            [[upcoming-policies.policies]]
            kind.volume = { factor = 0.002 }
            order-class = "any"
            "#,
        );
        let order = order(OrderKind::Sell, None);
        let at = |timestamp: &str| fees.apply(&order, timestamp.parse().unwrap());
        assert_eq!(
            at("2025-12-31T23:59:59Z"),
            vec![FeePolicy::Volume { factor: 0.001 }]
        );
        assert_eq!(
            at("2026-01-01T00:00:00Z"),
            vec![FeePolicy::Volume { factor: 0.002 }]
        );

        let empty_upcoming = from_toml(
            r#"
            [[policies]]
            kind.volume = { factor = 0.001 }
            order-class = "any"

            [upcoming-policies]
            effective-from-timestamp = "2026-01-01T00:00:00Z"
            "#,
        );
        assert_eq!(
            empty_upcoming.apply(&order, "2026-06-01T00:00:00Z".parse().unwrap()),
            vec![FeePolicy::Volume { factor: 0.001 }]
        );
    }

    #[test]
    fn no_config_means_no_policies() {
        assert!(
            ProtocolFees::default()
                .apply(&order(OrderKind::Sell, Some(quote(1, 1))), Utc::now())
                .is_empty()
        );
    }
}

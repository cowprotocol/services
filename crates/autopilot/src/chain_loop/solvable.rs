//! Solvable-orders filtering measured against the chain vocabulary.
//!
//! solvable_orders.rs is three different things at once: a pipeline
//! (fetch, filter, subtract, assemble, with drop bookkeeping), a set of
//! filter rules, and the chain services behind them. This module measures
//! how each generalizes:
//!
//! - the pipeline is chain-agnostic (one generic implementation below),
//! - set-membership filters (token deny list, banned users) are generic once
//!   the order exposes uid, owner, receiver and tokens,
//! - value filters (balance) keep a generic retain shape but the keep rule is
//!   chain semantics (EIP-1271 and flashloan exemptions vs SPL delegated
//!   amounts), one policy per chain.
//!
//! [`FilterChain`] is where the loop-level [`Chain`] vocabulary and the
//! logic-level [`ChainTypes`] vocabulary meet: filters need order ids
//! from the former and accounts, tokens and amounts from the latter.

use {
    super::Chain,
    async_trait::async_trait,
    std::{
        collections::{HashMap, HashSet},
        fmt::Debug,
        hash::Hash,
    },
    winner_selection::ChainTypes,
};

/// Order access the filters need, on top of the loop vocabulary.
pub trait FilterChain: Chain {
    /// The data-level type vocabulary (accounts, tokens, amounts).
    type Data: ChainTypes;
    /// The full order the filters inspect.
    type Order: Send + Sync;
    /// Why an order was dropped, the per-chain reason set.
    type Reason: Copy + Eq + Hash + Debug + Send + Sync;
    /// Key balances are fetched under (EVM: owner, token and source,
    /// Solana: the sell token account).
    type BalanceKey: Eq + Hash + Send + Sync;

    fn uid(order: &Self::Order) -> Self::OrderUid;
    fn owner(order: &Self::Order) -> <Self::Data as ChainTypes>::AccountId;
    fn receiver(order: &Self::Order) -> Option<<Self::Data as ChainTypes>::AccountId>;
    fn traded_tokens(order: &Self::Order) -> [<Self::Data as ChainTypes>::TokenId; 2];
    fn balance_key(order: &Self::Order) -> Self::BalanceKey;
}

/// One filter stage: names its drop reason and returns the uids to drop.
#[async_trait]
pub trait OrderFilter<C: FilterChain>: Send + Sync {
    fn reason(&self) -> C::Reason;
    async fn drops(&self, orders: &[C::Order]) -> Vec<C::OrderUid>;
}

pub struct Filtered<C: FilterChain> {
    pub kept: Vec<C::Order>,
    /// Every dropped order with the reason that claimed it first.
    pub dropped: Vec<(C::OrderUid, C::Reason)>,
}

/// The filtering pipeline of solvable_orders.rs `update`, generic: run
/// the stages in order over the surviving set, record one reason per
/// dropped order.
pub struct FilterPipeline<C: FilterChain> {
    filters: Vec<Box<dyn OrderFilter<C>>>,
    in_flight_reason: C::Reason,
}

impl<C: FilterChain> FilterPipeline<C> {
    pub fn new(filters: Vec<Box<dyn OrderFilter<C>>>, in_flight_reason: C::Reason) -> Self {
        Self {
            filters,
            in_flight_reason,
        }
    }

    /// In-flight orders are dropped first and claim their reason even if
    /// a later filter would also reject them, matching `update`, which
    /// erases invalid markings for in-flight orders.
    pub async fn run(
        &self,
        orders: Vec<C::Order>,
        in_flight: &HashSet<C::OrderUid>,
    ) -> Filtered<C> {
        let mut dropped = Vec::new();
        let mut kept = Vec::with_capacity(orders.len());
        for order in orders {
            if in_flight.contains(&C::uid(&order)) {
                dropped.push((C::uid(&order), self.in_flight_reason));
            } else {
                kept.push(order);
            }
        }

        for filter in &self.filters {
            let drops: HashSet<C::OrderUid> = filter.drops(&kept).await.into_iter().collect();
            if drops.is_empty() {
                continue;
            }
            kept.retain(|order| {
                let uid = C::uid(order);
                if drops.contains(&uid) {
                    dropped.push((uid, filter.reason()));
                    false
                } else {
                    true
                }
            });
        }

        Filtered { kept, dropped }
    }
}

/// Drops orders trading a deny-listed token. Fully generic.
pub struct TokenDenyList<C: FilterChain> {
    pub deny: HashSet<<C::Data as ChainTypes>::TokenId>,
    pub reason: C::Reason,
}

#[async_trait]
impl<C: FilterChain> OrderFilter<C> for TokenDenyList<C> {
    fn reason(&self) -> C::Reason {
        self.reason
    }

    async fn drops(&self, orders: &[C::Order]) -> Vec<C::OrderUid> {
        orders
            .iter()
            .filter(|order| {
                C::traded_tokens(order)
                    .iter()
                    .any(|token| self.deny.contains(token))
            })
            .map(|order| C::uid(order))
            .collect()
    }
}

/// The banned-address lookup. The set logic in [`BannedUsers`] is
/// generic, this seam is where the chain-specific backends live (EVM:
/// hardcoded list, Chainalysis oracle contract, Hermod HTTP).
#[async_trait]
pub trait BannedLookup<A>: Send + Sync {
    async fn banned(&self, candidates: Vec<A>) -> HashSet<A>;
}

/// Drops orders whose owner or receiver is banned. Fully generic given a
/// lookup.
pub struct BannedUsers<C: FilterChain> {
    pub lookup: Box<dyn BannedLookup<<C::Data as ChainTypes>::AccountId>>,
    pub reason: C::Reason,
}

#[async_trait]
impl<C: FilterChain> OrderFilter<C> for BannedUsers<C> {
    fn reason(&self) -> C::Reason {
        self.reason
    }

    async fn drops(&self, orders: &[C::Order]) -> Vec<C::OrderUid> {
        let candidates = orders
            .iter()
            .flat_map(|order| std::iter::once(C::owner(order)).chain(C::receiver(order)))
            .collect();
        let banned = self.lookup.banned(candidates).await;
        orders
            .iter()
            .filter(|order| {
                std::iter::once(C::owner(order))
                    .chain(C::receiver(order))
                    .any(|account| banned.contains(&account))
            })
            .map(|order| C::uid(order))
            .collect()
    }
}

/// The keep rule of the balance filter. This is where generality ends:
/// what makes an order fundable is chain semantics, so there is one
/// policy per chain and no shared implementation.
pub trait BalancePolicy<C: FilterChain>: Send + Sync {
    fn keep(&self, order: &C::Order, balance: Option<&<C::Data as ChainTypes>::Amount>) -> bool;
}

/// Balance filtering: the generic retain shape around a per-chain
/// policy. Exempt uids skip the check (EVM: orders with app-data
/// wrappers, unused on Solana).
pub struct BalanceFilter<C: FilterChain> {
    pub balances: HashMap<C::BalanceKey, <C::Data as ChainTypes>::Amount>,
    pub policy: Box<dyn BalancePolicy<C>>,
    pub exempt: HashSet<C::OrderUid>,
    pub reason: C::Reason,
}

#[async_trait]
impl<C: FilterChain> OrderFilter<C> for BalanceFilter<C> {
    fn reason(&self) -> C::Reason {
        self.reason
    }

    async fn drops(&self, orders: &[C::Order]) -> Vec<C::OrderUid> {
        orders
            .iter()
            .filter(|order| {
                !self.exempt.contains(&C::uid(order))
                    && !self
                        .policy
                        .keep(order, self.balances.get(&C::balance_key(order)))
            })
            .map(|order| C::uid(order))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{
            super::{
                evm::{EvmBalancePolicy, EvmBannedLookup, EvmChain},
                solana::{
                    SetBannedLookup,
                    SolanaBalancePolicy,
                    SolanaChain,
                    SolanaFilterReason,
                    SolanaSolvableOrder,
                },
            },
            *,
        },
        crate::{domain, infra::banned},
        account_balances::Query,
        alloy::primitives::{Address, U256},
        database::order_events::OrderFilterReason,
        model::{
            order::{Order, OrderData, OrderMetadata, OrderUid},
            signature::Signature,
        },
        std::collections::HashMap,
        winner_selection::solana::{IntentHash, Pubkey},
    };

    fn sol_order(uid: u8, owner: u8, receiver: Option<u8>) -> SolanaSolvableOrder {
        SolanaSolvableOrder {
            uid: IntentHash([uid; 32]),
            owner: Pubkey([owner; 32]),
            receiver: receiver.map(|r| Pubkey([r; 32])),
            sell_token: Pubkey([100; 32]),
            buy_token: Pubkey([101; 32]),
            sell_token_account: Pubkey([uid; 32]),
            sell_amount: 100,
            fee: 5,
            partially_fillable: false,
        }
    }

    fn evm_order(uid: u8, sell_token: u8, buy_token: u8) -> Order {
        Order {
            metadata: OrderMetadata {
                uid: OrderUid([uid; 56]),
                owner: Address::repeat_byte(uid),
                ..Default::default()
            },
            data: OrderData {
                sell_token: Address::repeat_byte(sell_token),
                buy_token: Address::repeat_byte(buy_token),
                sell_amount: U256::from(100u64),
                fee_amount: U256::from(5u64),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// An in-flight order that a filter would also reject is logged once,
    /// as in-flight, matching `update`'s invalid-marking erasure.
    #[tokio::test]
    async fn in_flight_claims_the_reason_first() {
        let banned_owner = 0xba;
        let pipeline = FilterPipeline::<SolanaChain>::new(
            vec![Box::new(BannedUsers::<SolanaChain> {
                lookup: Box::new(SetBannedLookup([Pubkey([banned_owner; 32])].into())),
                reason: SolanaFilterReason::BannedUser,
            })],
            SolanaFilterReason::InFlight,
        );
        let orders = vec![
            sol_order(1, banned_owner, None), // banned AND in flight
            sol_order(2, banned_owner, None), // banned only
            sol_order(3, 3, None),            // clean
        ];

        let result = pipeline.run(orders, &[IntentHash([1; 32])].into()).await;

        assert_eq!(
            result.kept.iter().map(|o| o.uid).collect::<Vec<_>>(),
            vec![IntentHash([3; 32])]
        );
        let mut dropped = result.dropped.clone();
        dropped.sort_by_key(|(uid, _)| uid.0);
        assert_eq!(
            dropped,
            vec![
                (IntentHash([1; 32]), SolanaFilterReason::InFlight),
                (IntentHash([2; 32]), SolanaFilterReason::BannedUser),
            ]
        );
    }

    /// One deny-list implementation serves both chains.
    #[tokio::test]
    async fn token_deny_list_is_the_same_code_on_both_chains() {
        let evm_filter = TokenDenyList::<EvmChain> {
            deny: [Address::repeat_byte(10)].into(),
            reason: OrderFilterReason::UnsupportedToken,
        };
        let evm_orders = vec![
            evm_order(1, 10, 11), // denied sell token
            evm_order(2, 11, 12),
            evm_order(3, 12, 10), // denied buy token
        ];
        assert_eq!(
            evm_filter.drops(&evm_orders).await,
            vec![domain::OrderUid([1; 56]), domain::OrderUid([3; 56]),]
        );

        let sol_filter = TokenDenyList::<SolanaChain> {
            deny: [Pubkey([100; 32])].into(),
            reason: SolanaFilterReason::UnsupportedToken,
        };
        let sol_orders = vec![sol_order(1, 1, None)]; // sells mint 100
        assert_eq!(
            sol_filter.drops(&sol_orders).await,
            vec![IntentHash([1; 32])]
        );
    }

    /// The owner-or-receiver rule is one implementation, the lookup
    /// backend is the seam: the real EVM `banned::Users` on one side, a
    /// plain set on the other.
    #[tokio::test]
    async fn banned_users_share_the_rule_not_the_backend() {
        let filter = BannedUsers::<EvmChain> {
            lookup: Box::new(EvmBannedLookup(banned::Users::from_set(
                [Address::repeat_byte(0xba)].into(),
            ))),
            reason: OrderFilterReason::BannedUser,
        };
        let mut receiver_case = evm_order(3, 1, 2);
        receiver_case.data.receiver = Some(Address::repeat_byte(0xba));
        let orders = vec![
            evm_order(1, 10, 11),
            evm_order(0xba, 10, 11), // banned owner
            receiver_case,           // banned receiver
        ];
        assert_eq!(
            filter.drops(&orders).await,
            vec![domain::OrderUid([0xba; 56]), domain::OrderUid([3; 56]),]
        );

        let filter = BannedUsers::<SolanaChain> {
            lookup: Box::new(SetBannedLookup([Pubkey([0xba; 32])].into())),
            reason: SolanaFilterReason::BannedUser,
        };
        let orders = vec![sol_order(1, 1, None), sol_order(2, 2, Some(0xba))];
        assert_eq!(filter.drops(&orders).await, vec![IntentHash([2; 32])]);
    }

    /// The keep rule is chain semantics: EVM exempts EIP-1271 and
    /// flashloan orders, Solana checks the SPL delegated amount.
    #[tokio::test]
    async fn balance_policies_are_chain_semantics() {
        let settlement_contract = Address::repeat_byte(0x55);

        let mut eip1271 = evm_order(1, 10, 11);
        eip1271.signature = Signature::Eip1271(vec![1]);
        let mut flashloan = evm_order(2, 10, 11);
        flashloan.data.receiver = Some(settlement_contract);
        let funded = evm_order(3, 10, 11);
        let broke = evm_order(4, 10, 11);

        let wrapper_exempt = evm_order(5, 10, 11);
        let filter = BalanceFilter::<EvmChain> {
            balances: HashMap::from([(Query::from_order(&funded), U256::from(105u64))]),
            policy: Box::new(EvmBalancePolicy {
                settlement_contract,
            }),
            exempt: [domain::OrderUid([5; 56])].into(),
            reason: OrderFilterReason::InsufficientBalance,
        };
        // EIP-1271, flashloan and exempt (app-data wrapper) orders survive
        // with no balance at all, the funded order covers sell + fee, the
        // last one has nothing.
        assert_eq!(
            filter
                .drops(&[eip1271, flashloan, wrapper_exempt, funded, broke])
                .await,
            vec![domain::OrderUid([4; 56])]
        );

        let fok_underfunded = sol_order(1, 1, None); // needs 105 delegated
        let mut partial = sol_order(2, 2, None);
        partial.partially_fillable = true;
        let balances = HashMap::from([
            (Pubkey([1; 32]), 104u64), // one lamport short of sell + fee
            (Pubkey([2; 32]), 1u64),   // enough for a partial fill
        ]);
        let filter = BalanceFilter::<SolanaChain> {
            balances,
            policy: Box::new(SolanaBalancePolicy),
            exempt: HashSet::new(),
            reason: SolanaFilterReason::InsufficientBalance,
        };
        assert_eq!(
            filter.drops(&[fok_underfunded, partial]).await,
            vec![IntentHash([1; 32])]
        );
    }
}

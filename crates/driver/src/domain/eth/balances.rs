use {
    crate::{
        domain::{competition::order, eth},
        infra,
    },
    futures::future::join_all,
    itertools::Itertools,
    std::{collections::HashMap, sync::Arc},
    tokio::sync::Mutex,
};

type BalanceGroup = (order::Trader, eth::TokenAddress, order::SellTokenBalance);
type Balances = Arc<HashMap<BalanceGroup, order::SellAmount>>;

#[derive(Default, Clone)]
pub struct Cache(Arc<Mutex<Inner>>);

impl Cache {
    /// Either returns the cached balances if they are up-to-date or fetches,
    /// caches and returns the current balances.
    /// If the return value does not contain a balance for a given order it
    /// could not be fetched.
    pub async fn get_or_fetch(
        &self,
        ethereum: &infra::Ethereum,
        orders: &[order::Order],
    ) -> Balances {
        let mut lock = self.0.lock().await;
        let current_block = ethereum.current_block().borrow().number;
        if lock.cached_at.0 >= current_block {
            // Check if somebody else already filled the cache by now.
            return lock.balances.clone();
        }

        // Collect trader/token/source/interaction tuples for fetching available
        // balances. Note that we are pessimistic here, if a trader is selling
        // the same token with the same source in two different orders using a
        // different set of pre-interactions, then we fetch the balance as if no
        // pre-interactions were specified. This is done to avoid creating
        // dependencies between orders (i.e. order 1 is required for executing
        // order 2) which we currently cannot express with the solver interface.
        let traders = orders
            .iter()
            .group_by(|order| (order.trader(), order.sell.token, order.sell_token_balance))
            .into_iter()
            .map(|((trader, token, source), mut orders)| {
                let first = orders.next().expect("group contains at least 1 order");
                let mut others = orders;
                if others.all(|order| order.pre_interactions == first.pre_interactions) {
                    (trader, token, source, &first.pre_interactions[..])
                } else {
                    (trader, token, source, Default::default())
                }
            })
            .collect::<Vec<_>>();

        let balances = join_all(traders.into_iter().map(
            |(trader, token, source, interactions)| async move {
                let balance = ethereum
                    .erc20(token)
                    .tradable_balance(trader.into(), source, interactions)
                    .await;
                (
                    (trader, token, source),
                    balance.map(order::SellAmount::from).ok(),
                )
            },
        ))
        .await
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
        .collect::<HashMap<_, _>>();

        // Cache new balances.
        let balances = Arc::new(balances);
        lock.cached_at = eth::BlockNo(current_block);
        lock.balances = balances.clone();

        balances
    }
}

struct Inner {
    cached_at: eth::BlockNo,
    balances: Balances,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            cached_at: eth::BlockNo(0),
            balances: Default::default(),
        }
    }
}

use {
    alloy_primitives::{Address, U256},
    std::{
        collections::HashMap,
        sync::atomic::{AtomicU64, Ordering},
    },
    tokio::sync::RwLock,
};

#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: u64,
    pub order_uid: [u8; 56],
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub interactions: Vec<Interaction>,
    pub solver: Address,
    pub valid_until: u64,
    pub nonce: U256,
}

#[serde_with::serde_as]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interaction {
    pub target: Address,
    #[serde_as(as = "number::serialization::HexOrDecimalU256")]
    pub value: U256,
    #[serde_as(as = "serde_ext::Hex")]
    pub calldata: Vec<u8>,
}

pub struct ProposalStore {
    proposals: RwLock<HashMap<[u8; 56], Vec<Proposal>>>,
    next_id: AtomicU64,
}

impl ProposalStore {
    pub fn new() -> Self {
        Self {
            proposals: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    pub async fn insert(&self, mut proposal: Proposal) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        proposal.id = id;
        let order_uid = proposal.order_uid;
        self.proposals
            .write()
            .await
            .entry(order_uid)
            .or_default()
            .push(proposal);
        id
    }

    /// Returns the best proposal for the given order UID (highest
    /// buy_amount/sell_amount ratio = best surplus for the user).
    pub async fn get_best(&self, order_uid: &[u8; 56]) -> Option<Proposal> {
        let proposals = self.proposals.read().await;
        let candidates = proposals.get(order_uid)?;
        let now = chrono::Utc::now().timestamp() as u64;
        candidates
            .iter()
            .filter(|p| p.valid_until > now)
            .max_by(|a, b| {
                // Compare surplus: a.buy/a.sell vs b.buy/b.sell
                // Cross-multiply to avoid division: a.buy * b.sell vs b.buy * a.sell
                let lhs = a.buy_amount.wrapping_mul(b.sell_amount);
                let rhs = b.buy_amount.wrapping_mul(a.sell_amount);
                lhs.cmp(&rhs)
            })
            .cloned()
    }

    pub async fn get_metadata(&self, order_uid: &[u8; 56]) -> Option<ProposalMetadata> {
        let proposals = self.proposals.read().await;
        let candidates = proposals.get(order_uid)?;
        let now = chrono::Utc::now().timestamp() as u64;
        let active: Vec<_> = candidates.iter().filter(|p| p.valid_until > now).collect();
        if active.is_empty() {
            return None;
        }
        Some(ProposalMetadata {
            count: active.len(),
        })
    }

    pub async fn remove(&self, id: u64) -> bool {
        let mut proposals = self.proposals.write().await;
        for candidates in proposals.values_mut() {
            if let Some(pos) = candidates.iter().position(|p| p.id == id) {
                candidates.remove(pos);
                return true;
            }
        }
        false
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ProposalMetadata {
    pub count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_and_get_best() {
        let store = ProposalStore::new();
        let uid = [0u8; 56];

        let p1 = Proposal {
            id: 0,
            order_uid: uid,
            sell_amount: U256::from(1000),
            buy_amount: U256::from(2000),
            interactions: vec![],
            solver: Address::ZERO,
            valid_until: u64::MAX,
            nonce: U256::ZERO,
        };
        let p2 = Proposal {
            id: 0,
            order_uid: uid,
            sell_amount: U256::from(1000),
            buy_amount: U256::from(3000), // better surplus
            interactions: vec![],
            solver: Address::ZERO,
            valid_until: u64::MAX,
            nonce: U256::from(1),
        };

        store.insert(p1).await;
        store.insert(p2).await;

        let best = store.get_best(&uid).await.unwrap();
        assert_eq!(best.buy_amount, U256::from(3000));
    }

    #[tokio::test]
    async fn expired_proposals_filtered() {
        let store = ProposalStore::new();
        let uid = [0u8; 56];

        let p = Proposal {
            id: 0,
            order_uid: uid,
            sell_amount: U256::from(1000),
            buy_amount: U256::from(2000),
            interactions: vec![],
            solver: Address::ZERO,
            valid_until: 0, // already expired
            nonce: U256::ZERO,
        };
        store.insert(p).await;
        assert!(store.get_best(&uid).await.is_none());
    }

    #[tokio::test]
    async fn remove_proposal() {
        let store = ProposalStore::new();
        let uid = [0u8; 56];

        let p = Proposal {
            id: 0,
            order_uid: uid,
            sell_amount: U256::from(1000),
            buy_amount: U256::from(2000),
            interactions: vec![],
            solver: Address::ZERO,
            valid_until: u64::MAX,
            nonce: U256::ZERO,
        };
        let id = store.insert(p).await;
        assert!(store.remove(id).await);
        assert!(store.get_best(&uid).await.is_none());
    }
}

use {
    crate::{cache::Storage, Amm},
    contracts::ERC20,
    ethcontract::futures::future::{join_all, select_ok},
    ethrpc::Web3,
    shared::maintenance::Maintaining,
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub struct EmptyPoolRemoval {
    storage: Arc<RwLock<Vec<Storage>>>,
    web3: Web3,
}

impl EmptyPoolRemoval {
    pub fn new(storage: Arc<RwLock<Vec<Storage>>>, web3: Web3) -> Self {
        Self { storage, web3 }
    }

    /// Checks if the given AMM has a zero token balance.
    async fn has_zero_balance(&self, amm: Arc<Amm>) -> bool {
        let amm_address = amm.address();
        let futures = amm
            .traded_tokens()
            .iter()
            .map(move |token| async move {
                ERC20::at(&self.web3, *token)
                    .balance_of(*amm_address)
                    .call()
                    .await
                    .map_err(|err| {
                        tracing::warn!(
                            amm = ?amm_address,
                            ?token,
                            ?err,
                            "failed to check AMM token balance"
                        );
                    })
                    .and_then(|balance| if balance.is_zero() { Ok(()) } else { Err(()) })
            })
            .map(Box::pin);
        // If any future resolved to Ok(()), then there exists a zero balance.
        select_ok(futures).await.is_ok()
    }
}

#[async_trait::async_trait]
impl Maintaining for EmptyPoolRemoval {
    async fn run_maintenance(&self) -> anyhow::Result<()> {
        let mut amms_to_check = Vec::<Arc<Amm>>::new();
        {
            let lock = self.storage.read().await;
            for storage in lock.iter() {
                amms_to_check.extend(storage.cow_amms().await);
            }
        }
        let futures = amms_to_check.iter().map(|amm| async {
            self.has_zero_balance(amm.clone())
                .await
                .then_some(*amm.address())
        });

        let empty_amms: Vec<_> = join_all(futures).await.into_iter().flatten().collect();
        if !empty_amms.is_empty() {
            tracing::debug!(amms = ?empty_amms, "removing AMMs with zero token balance");
            let lock = self.storage.read().await;
            join_all(
                lock.iter()
                    .map(|storage| async { storage.remove_amms(&empty_amms).await }),
            )
            .await;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "EmptyPoolRemoval"
    }
}

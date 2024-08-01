use {
    crate::{cache::Storage, Amm},
    contracts::ERC20,
    ethcontract::{errors::MethodError, futures::future::join_all, Address},
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

    /// Checks if the given AMM has a zero balance of the specified token.
    async fn check_balance(
        &self,
        token: Address,
        amm_address: Address,
    ) -> Result<bool, MethodError> {
        ERC20::at(&self.web3, token)
            .balance_of(amm_address)
            .call()
            .await
            .map(|balance| balance.is_zero())
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
        let futures = amms_to_check.iter().flat_map(|amm| {
            let amm_address = amm.address();
            amm.traded_tokens().iter().map(move |token| async move {
                match self.check_balance(*token, *amm_address).await {
                    Ok(is_empty) => is_empty.then_some(*amm_address),
                    Err(err) => {
                        tracing::warn!(
                            amm = ?amm_address,
                            ?token,
                            ?err,
                            "failed to check AMM token balance"
                        );
                        None
                    }
                }
            })
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

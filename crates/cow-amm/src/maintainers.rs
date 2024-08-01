use {
    crate::cache::Storage,
    contracts::ERC20,
    ethcontract::{errors::MethodError, futures::future::join_all, Address},
    ethrpc::Web3,
    shared::maintenance::Maintaining,
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
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
    async fn check_single(
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
        let mut amms_to_check = HashMap::<Address, HashSet<Address>>::new();
        {
            let lock = self.storage.read().await;
            for storage in lock.iter() {
                for amm in storage.cow_amms().await {
                    amms_to_check
                        .entry(*amm.address())
                        .or_default()
                        .extend(amm.traded_tokens())
                }
            }
        }
        let futures = amms_to_check.into_iter().flat_map(|(amm_address, tokens)| {
            tokens.into_iter().map(move |token| async move {
                match self.check_single(token, amm_address).await {
                    Ok(is_empty) => is_empty.then_some(amm_address),
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

        let empty_amms: HashSet<Address> = join_all(futures).await.into_iter().flatten().collect();
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

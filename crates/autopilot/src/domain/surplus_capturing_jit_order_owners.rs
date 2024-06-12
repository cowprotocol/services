use {
    crate::domain::eth::Address,
    contracts::cow_amm_constant_product_factory::Event as CowAMMProductFactoryEvent,
    primitive_types::H160,
    std::{collections::HashSet, sync::Arc},
    tokio::sync::{broadcast, RwLock},
};

/// Surplus capturing JIT order owners
/// The list of owners is initialized with the values specified in the autopilot
/// configuration file, and it is updated with all the CoW AMM owners which are
/// deployed by the CoW AMM product factory contract by listening to the events
/// emitted by such contract
pub struct SurplusCapturingJitOrderOwners {
    /// The order owners that capture surplus for JIT orders
    surplus_capturing_jit_order_owners: Arc<RwLock<HashSet<Address>>>,
}

impl SurplusCapturingJitOrderOwners {
    pub fn new(
        surplus_capturing_jit_order_owners_configuration: &[H160],
        receiver: Option<
            broadcast::Receiver<
                ethcontract::Event<contracts::cow_amm_constant_product_factory::Event>,
            >,
        >,
    ) -> Self {
        let inner = Self {
            surplus_capturing_jit_order_owners: Arc::new(RwLock::new(
                surplus_capturing_jit_order_owners_configuration
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect(),
            )),
        };
        if let Some(receiver) = receiver {
            inner.listen_for_updates(receiver);
        }
        inner
    }

    pub async fn is_surplus_capturing_jit_order_owner(&self, owner: &Address) -> bool {
        self.surplus_capturing_jit_order_owners
            .read()
            .await
            .contains(owner)
    }

    pub async fn get_all(&self) -> Vec<Address> {
        let owners = self.surplus_capturing_jit_order_owners.read().await;
        owners.clone().into_iter().collect()
    }

    fn listen_for_updates(
        &self,
        mut receiver: broadcast::Receiver<ethcontract::Event<CowAMMProductFactoryEvent>>,
    ) {
        let surplus_capturing_jit_order_owners = self.surplus_capturing_jit_order_owners.clone();
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        if let CowAMMProductFactoryEvent::Deployed(ref deployed_event) = event.data
                        {
                            let mut owners = surplus_capturing_jit_order_owners.write().await;
                            owners.insert(deployed_event.amm.into());
                        }
                    }
                    Err(e) => {
                        tracing::error!(?e, "error receiving a CoW AMM product factory event");
                    }
                }
            }
        });
    }
}

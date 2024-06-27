use {
    anyhow::Result,
    ethcontract::{Address, U256},
    model::{interaction::InteractionData, order::OrderData, signature::Signature},
    contracts::CowAmmLegacyHelper,
};

#[derive(Clone)]
pub(crate) struct Amm {
    helper: contracts::CowAmmLegacyHelper,
    address: Address,
    tradeable_tokens: Vec<Address>,
}

impl Amm {
    pub(crate) async fn new(address: Address, helper: &CowAmmLegacyHelper) -> Result<Self> {
        let tradeable_tokens = helper.tokens(address).call().await?;

        Ok(Self {
            helper: helper.clone(),
            address,
            tradeable_tokens,
        })
    }
}

#[async_trait::async_trait]
impl crate::CowAmm for Amm {
    fn address(&self) -> &Address {
        &self.address
    }

    fn traded_tokens(&self) -> &[Address] {
        &self.tradeable_tokens
    }

    async fn template_order(
        &self,
        prices: Vec<U256>,
    ) -> Result<(
        OrderData,
        Signature,
        Vec<InteractionData>,
        Vec<InteractionData>,
    )> {
        let (order, pre_interactions, post_interactions, signature) =
            self.helper.order(self.address, prices).call().await?;
        self.convert_orders_reponse(order, signature, pre_interactions, post_interactions)
    }
}

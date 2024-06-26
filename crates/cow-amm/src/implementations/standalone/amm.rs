use {
    anyhow::Result,
    ethcontract::{Address, U256},
    model::{interaction::InteractionData, order::OrderData, signature::Signature},
};

#[derive(Clone)]
pub(crate) struct Amm {
    address: Address,
    tradeable_tokens: [Address; 2],
}

impl Amm {
    pub(crate) fn new(address: Address, tradeable_tokens: [Address; 2]) -> Self {
        Self {
            address,
            tradeable_tokens,
        }
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
        _prices: &[U256],
    ) -> Result<(OrderData, Signature, InteractionData)> {
        anyhow::bail!("not implemented")
    }
}

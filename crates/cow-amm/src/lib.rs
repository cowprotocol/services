mod implementations;
mod registry;

use {
    anyhow::Result,
    app_data::AppDataHash,
    ethcontract::{Address, Bytes, U256},
    ethrpc::Web3,
    model::{
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::Signature,
    },
    std::sync::Arc,
};
pub use {
    implementations::standalone::factory::Contract as CowAmmStandaloneFactory,
    registry::Registry,
};

#[async_trait::async_trait]
pub trait Deployment: Sync + Send {
    /// Returns the AMM deployed in the given Event.
    async fn deployed_amm(&self, web3: &Web3) -> Result<Option<Arc<dyn CowAmm>>>;
}

#[async_trait::async_trait]
pub trait CowAmm: Send + Sync {
    /// Address of the CoW AMM.
    fn address(&self) -> &Address;

    /// Returns all tokens traded by this pool in stable order.
    fn traded_tokens(&self) -> &[Address];

    /// Returns an order to rebalance the AMM based on the provided reference
    /// prices. `prices` need to be computed using a common denominator and
    /// need to be supplied in the same order as `traded_tokens` returns
    /// token addresses.
    async fn template_order(
        &self,
        prices: Vec<U256>,
    ) -> Result<(
        OrderData,
        Signature,
        Vec<InteractionData>,
        Vec<InteractionData>,
    )>;

    /// Converts a successful response of the CowAmmHelper into domain types.
    fn convert_orders_reponse(
        &self,
        order: RawOrder,
        signature: Bytes<Vec<u8>>,
        pre_interactions: Vec<RawInteraction>,
        post_interactions: Vec<RawInteraction>,
    ) -> Result<(
        OrderData,
        Signature,
        Vec<InteractionData>,
        Vec<InteractionData>,
    )> {
        let order = OrderData {
            sell_token: order.0,
            buy_token: order.1,
            receiver: Some(order.2),
            sell_amount: order.3,
            buy_amount: order.4,
            valid_to: order.5,
            app_data: AppDataHash(order.6 .0),
            fee_amount: order.7,
            // order.8
            kind: if order.8 .0
                == *hex::decode("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775")
                    .unwrap()
            {
                OrderKind::Sell
            } else {
                OrderKind::Buy
            },
            partially_fillable: order.9,
            //order.10
            sell_token_balance: SellTokenSource::Erc20,
            //order.11
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        tracing::error!(?order);

        let pre_interactions = pre_interactions
            .into_iter()
            .map(convert_interaction)
            .collect();
        let post_interactions = post_interactions
            .into_iter()
            .map(convert_interaction)
            .collect();

        // Prepend amm address so the settlement contract knows which contract
        // this signature belongs to.
        let signature = [self.address().as_bytes(), &signature.0].concat();
        let signature = Signature::Eip1271(signature);

        Ok((order, signature, pre_interactions, post_interactions))
    }
}

fn convert_interaction(interaction: RawInteraction) -> InteractionData {
    InteractionData {
        target: interaction.0,
        value: interaction.1,
        call_data: interaction.2 .0,
    }
}

type RawOrder = (
    Address,
    Address,
    Address,
    U256,
    U256,
    u32,
    Bytes<[u8; 32]>,
    U256,
    Bytes<[u8; 32]>,
    bool,
    Bytes<[u8; 32]>,
    Bytes<[u8; 32]>,
);

type RawInteraction = (Address, U256, Bytes<Vec<u8>>);

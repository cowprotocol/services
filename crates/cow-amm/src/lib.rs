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
            kind: convert_kind(&order.8 .0)?,
            partially_fillable: order.9,
            sell_token_balance: convert_sell_token_source(&order.10 .0)?,
            buy_token_balance: convert_buy_token_destination(&order.11 .0)?,
        };

        let pre_interactions = pre_interactions
            .into_iter()
            .map(convert_interaction)
            .collect();
        let post_interactions = post_interactions
            .into_iter()
            .map(convert_interaction)
            .collect();

        let signature = Signature::Eip1271(signature.0);

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

// Hex strings for enums have been copied from
// <https://github.com/cowprotocol/contracts/blob/main/src/contracts/libraries/GPv2Order.sol#L50>

fn convert_kind(bytes: &[u8]) -> Result<OrderKind> {
    match hex::encode(bytes).as_str() {
        "f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775" => Ok(OrderKind::Sell),
        "6ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc" => Ok(OrderKind::Buy),
        bytes => anyhow::bail!("unknown order type: {bytes}"),
    }
}

const BALANCE_ERC20: &str = "5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9";
const BALANCE_INTERNAL: &str = "4ac99ace14ee0a5ef932dc609df0943ab7ac16b7583634612f8dc35a4289a6ce";
const BALANCE_EXTERNAL: &str = "abee3b73373acd583a130924aad6dc38cfdc44ba0555ba94ce2ff63980ea0632";

fn convert_sell_token_source(bytes: &[u8]) -> Result<SellTokenSource> {
    match hex::encode(bytes).as_str() {
        BALANCE_ERC20 => Ok(SellTokenSource::Erc20),
        BALANCE_INTERNAL => Ok(SellTokenSource::Internal),
        BALANCE_EXTERNAL => Ok(SellTokenSource::External),
        bytes => anyhow::bail!("unknown sell token source: {bytes}"),
    }
}

fn convert_buy_token_destination(bytes: &[u8]) -> Result<BuyTokenDestination> {
    match hex::encode(bytes).as_str() {
        BALANCE_ERC20 => Ok(BuyTokenDestination::Erc20),
        BALANCE_INTERNAL => Ok(BuyTokenDestination::Internal),
        bytes => anyhow::bail!("unknown buy token destination: {bytes}"),
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

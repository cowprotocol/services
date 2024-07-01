use {
    anyhow::Result,
    app_data::AppDataHash,
    contracts::CowAmmLegacyHelper,
    ethcontract::{Address, Bytes, U256},
    model::{
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::Signature,
    },
};

#[derive(Clone, Debug)]
pub struct Amm {
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

    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Returns all tokens traded by this pool in stable order.
    pub fn traded_tokens(&self) -> &[Address] {
        &self.tradeable_tokens
    }

    /// Returns an order to rebalance the AMM based on the provided reference
    /// prices. `prices` need to be computed using a common denominator and
    /// need to be supplied in the same order as `traded_tokens` returns
    /// token addresses.
    pub async fn template_order(&self, prices: Vec<U256>) -> Result<TemplateOrder> {
        let (order, pre_interactions, post_interactions, signature) =
            self.helper.order(self.address, prices).call().await?;
        self.convert_orders_reponse(order, signature, pre_interactions, post_interactions)
    }

    /// Converts a successful response of the CowAmmHelper into domain types.
    /// Can be used for any contract that correctly implements the CoW AMM
    /// helper interface.
    fn convert_orders_reponse(
        &self,
        order: RawOrder,
        signature: Bytes<Vec<u8>>,
        pre_interactions: Vec<RawInteraction>,
        post_interactions: Vec<RawInteraction>,
    ) -> Result<TemplateOrder> {
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

        let pre_interactions = convert_interactions(pre_interactions);
        let post_interactions = convert_interactions(post_interactions);

        let signature = Signature::Eip1271(signature.0);

        Ok(TemplateOrder {
            order,
            signature,
            pre_interactions,
            post_interactions,
        })
    }
}

/// Order suggested by a CoW AMM helper contract to rebalance the AMM according
/// to an external price vector.
pub struct TemplateOrder {
    /// CoW protocol order that should be executed.
    pub order: OrderData,
    /// Signature for the given order.
    pub signature: Signature,
    /// Transactions to be executed before transfering funds into the settlement
    /// contract.
    pub pre_interactions: Vec<InteractionData>,
    /// Transactions to be executed after transfering funds out of the
    /// settlement contract.
    pub post_interactions: Vec<InteractionData>,
}

fn convert_interactions(interactions: Vec<RawInteraction>) -> Vec<InteractionData> {
    interactions
        .into_iter()
        .map(|interaction| InteractionData {
            target: interaction.0,
            value: interaction.1,
            call_data: interaction.2 .0,
        })
        .collect()
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

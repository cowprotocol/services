use {
    alloy::primitives::{Address, TxHash, U256},
    anyhow::{Context, Result},
    app_data::AppDataHash,
    contracts::alloy::cow_amm::{
        CowAmmLegacyHelper,
        CowAmmLegacyHelper::CowAmmLegacyHelper::orderReturn,
    },
    database::byte_array::ByteArray,
    model::{
        DomainSeparator,
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::{Signature, hashed_eip712_message},
    },
    shared::signature_validator::{SignatureCheck, SignatureValidating},
};

#[derive(Clone, Debug)]
pub struct Amm {
    helper: CowAmmLegacyHelper::Instance,
    address: Address,
    tradeable_tokens: Vec<Address>,
}

impl Amm {
    pub async fn new(
        address: Address,
        helper: &CowAmmLegacyHelper::Instance,
    ) -> alloy::contract::Result<Self> {
        let tradeable_tokens = helper.tokens(address).call().await?;

        Ok(Self {
            address,
            helper: helper.clone(),
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
        let order_return = self.helper.order(self.address, prices).call().await?;
        self.convert_orders_reponse(order_return)
    }

    /// Generates a template order to rebalance the AMM but also verifies that
    /// the signature is actually valid to protect against buggy helper
    /// contracts.
    pub async fn validated_template_order(
        &self,
        prices: Vec<U256>,
        validator: &dyn SignatureValidating,
        domain_separator: &DomainSeparator,
    ) -> Result<TemplateOrder> {
        let template = self.template_order(prices).await?;

        // A buggy helper contract could return a signature that is actually not valid.
        // To avoid issues caused by that we check the validity of the signature.
        let hash = hashed_eip712_message(domain_separator, &template.order.hash_struct());
        validator
            .validate_signature_and_get_additional_gas(SignatureCheck {
                signer: self.address,
                hash: hash.0,
                signature: template.signature.to_bytes(),
                interactions: template.pre_interactions.clone(),
                balance_override: None,
            })
            .await
            .context("invalid signature")?;

        Ok(template)
    }

    pub fn try_to_db_type(
        &self,
        block_number: u64,
        factory_address: Address,
        tx_hash: TxHash,
    ) -> Result<database::cow_amms::CowAmm> {
        Ok(database::cow_amms::CowAmm {
            address: ByteArray(self.address.0.0),
            factory_address: ByteArray(factory_address.0.0),
            tradeable_tokens: self
                .tradeable_tokens
                .iter()
                .cloned()
                .map(|addr| ByteArray(addr.0.0))
                .collect(),
            block_number: i64::try_from(block_number)
                .with_context(|| format!("block number {block_number} is not i64"))?,
            tx_hash: ByteArray(tx_hash.0),
        })
    }

    /// Converts a successful response of the CowAmmHelper into domain types.
    /// Can be used for any contract that correctly implements the CoW AMM
    /// helper interface.
    fn convert_orders_reponse(&self, order_return: orderReturn) -> Result<TemplateOrder> {
        let order = OrderData {
            sell_token: order_return._order.sellToken,
            buy_token: order_return._order.buyToken,
            receiver: Some(order_return._order.receiver),
            sell_amount: order_return._order.sellAmount,
            buy_amount: order_return._order.buyAmount,
            valid_to: order_return._order.validTo,
            app_data: AppDataHash(order_return._order.appData.0),
            fee_amount: order_return._order.feeAmount,
            kind: convert_kind(&order_return._order.kind.0)?,
            partially_fillable: order_return._order.partiallyFillable,
            sell_token_balance: convert_sell_token_source(&order_return._order.sellTokenBalance.0)?,
            buy_token_balance: convert_buy_token_destination(
                &order_return._order.buyTokenBalance.0,
            )?,
        };

        let pre_interactions = convert_interactions(order_return.preInteractions);
        let post_interactions = convert_interactions(order_return.postInteractions);

        // The settlement contract expects a signature composed of 2 parts: the
        // signer address and the actual signature bytes.
        // The helper contract returns exactly that format but in our code base we
        // expect the signature to not already include the signer address (the parts
        // will be concatenated in the encoding logic) so we discard the first 20 bytes.
        let raw_signature = order_return.sig.0.into_iter().skip(20).collect();
        let signature = Signature::Eip1271(raw_signature);

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

fn convert_interactions(
    interactions: Vec<CowAmmLegacyHelper::GPv2Interaction::Data>,
) -> Vec<InteractionData> {
    interactions
        .into_iter()
        .map(|interaction| InteractionData {
            target: interaction.target,
            value: interaction.value,
            call_data: interaction.callData.to_vec(),
        })
        .collect()
}

// Hex strings for enums have been copied from
// <https://github.com/cowprotocol/contracts/blob/main/src/contracts/libraries/GPv2Order.sol#L50>

fn convert_kind(bytes: &[u8]) -> Result<OrderKind> {
    match const_hex::encode(bytes).as_str() {
        "f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775" => Ok(OrderKind::Sell),
        "6ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc" => Ok(OrderKind::Buy),
        bytes => anyhow::bail!("unknown order type: {bytes}"),
    }
}

const BALANCE_ERC20: &str = "5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9";
const BALANCE_INTERNAL: &str = "4ac99ace14ee0a5ef932dc609df0943ab7ac16b7583634612f8dc35a4289a6ce";
const BALANCE_EXTERNAL: &str = "abee3b73373acd583a130924aad6dc38cfdc44ba0555ba94ce2ff63980ea0632";

fn convert_sell_token_source(bytes: &[u8]) -> Result<SellTokenSource> {
    match const_hex::encode(bytes).as_str() {
        BALANCE_ERC20 => Ok(SellTokenSource::Erc20),
        BALANCE_INTERNAL => Ok(SellTokenSource::Internal),
        BALANCE_EXTERNAL => Ok(SellTokenSource::External),
        bytes => anyhow::bail!("unknown sell token source: {bytes}"),
    }
}

fn convert_buy_token_destination(bytes: &[u8]) -> Result<BuyTokenDestination> {
    match const_hex::encode(bytes).as_str() {
        BALANCE_ERC20 => Ok(BuyTokenDestination::Erc20),
        BALANCE_INTERNAL => Ok(BuyTokenDestination::Internal),
        bytes => anyhow::bail!("unknown buy token destination: {bytes}"),
    }
}

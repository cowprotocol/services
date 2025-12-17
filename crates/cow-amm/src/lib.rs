mod amm;
mod cache;
mod factory;
mod maintainers;
mod registry;

pub use {
    amm::Amm,
    contracts::alloy::cow_amm::CowAmmLegacyHelper::Instance as Helper,
    registry::Registry,
};

#[derive(prometheus_metric_storage::MetricStorage)]
pub(crate) struct Metrics {
    /// How log db queries take.
    #[metric(name = "cow_amm_database_queries", labels("type"))]
    database_queries: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

/// GPv2Order-specific signing utilities.
///
/// CoW Protocol uses GPv2Order structs for order representation. For EIP-712
/// signing, certain fields use `string` types in the type hash function (for
/// better UX) but the same fields are stored on-chain as `bytes32`.
///
/// See: <https://github.com/cowprotocol/contracts/blob/v1.1.2/src/contracts/libraries/GPv2Order.sol#L26-L48>
pub mod gpv2_order {
    use {
        alloy::{
            primitives::{B256, Keccak256},
            sol_types::{SolStruct, SolValue},
        },
        contracts::alloy::cow_amm::CowAmm,
        model::{DomainSeparator, interaction::InteractionData, signature::hashed_eip712_message},
    };

    /// The correct EIP-712 type hash for GPv2Order as defined in CoW Protocol
    /// contracts.
    ///
    /// This corresponds to:
    /// ```text
    /// keccak256("Order(address sellToken,address buyToken,address receiver,uint256 sellAmount,uint256 buyAmount,uint32 validTo,bytes32 appData,uint256 feeAmount,string kind,bool partiallyFillable,string sellTokenBalance,string buyTokenBalance)")
    /// ```
    ///
    /// Note the use of `string` for kind, sellTokenBalance, and buyTokenBalance
    /// instead of `bytes32`.
    const TYPE_HASH: [u8; 32] =
        alloy::hex!("d5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e489");

    /// Computes the correct EIP-712 hash for a GPv2Order.
    fn eip712_hash_struct(order: &CowAmm::GPv2Order::Data) -> B256 {
        let mut hasher = Keccak256::new();
        hasher.update(TYPE_HASH);
        hasher.update(order.eip712_encode_data());
        hasher.finalize()
    }

    /// Generates an EIP-1271 signature for a CoW AMM GPv2Order.
    ///
    /// The signature format is:
    /// 1. AMM address (20 bytes)
    /// 2. ABI-encoded order data and trading parameters
    ///
    /// # Returns
    /// The complete signature bytes that can be verified by the CoW Protocol
    /// settlement contract.
    pub fn generate_eip1271_signature(
        order: &CowAmm::GPv2Order::Data,
        trading_params: &CowAmm::ConstantProduct::TradingParams,
        amm_address: alloy::primitives::Address,
    ) -> Vec<u8> {
        // Encode the order and trading params
        let signature_data = (order.clone(), trading_params.clone()).abi_encode_sequence();

        // Prepend AMM address to the signature
        amm_address
            .as_slice()
            .iter()
            .copied()
            .chain(signature_data)
            .collect()
    }

    /// Generates a commit interaction for a CoW AMM GPv2Order.
    ///
    /// The commit interaction ensures that only the specified order can be
    /// settled in the current CoW Protocol batch.
    pub fn generate_commit_interaction(
        order: &CowAmm::GPv2Order::Data,
        amm: &CowAmm::Instance,
        domain_separator: &DomainSeparator,
    ) -> InteractionData {
        let order_hash = eip712_hash_struct(order);
        let order_hash = hashed_eip712_message(domain_separator, &order_hash);
        let calldata = amm.commit(order_hash).calldata().clone();

        InteractionData {
            target: *amm.address(),
            value: Default::default(),
            call_data: calldata.to_vec(),
        }
    }
}

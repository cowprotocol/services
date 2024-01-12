use {
    crate::domain::fee,
    primitive_types::{H160, H256, U256},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub uid: OrderUid,
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub user_fee: U256,
    // Same as user_fee, but without subsidies. This value should be used to score solutions.
    pub scoring_fee: U256,
    pub kind: Kind,
    pub class: Class,
    pub valid_to: u32,
    pub receiver: Option<H160>,
    pub owner: H160,
    pub partially_fillable: bool,
    pub executed: U256,
    // Partially fillable orders should have their pre-interactions only executed
    // on the first fill.
    pub pre_interactions: Vec<Interaction>,
    pub post_interactions: Vec<Interaction>,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub app_data: AppDataHash,
    pub signature: Signature,
    pub fee_policies: Vec<fee::Policy>,
}

// uid as 56 bytes: 32 for orderDigest, 20 for ownerAddress and 4 for validTo
#[derive(Copy, Clone, Debug, PartialEq, Hash, Eq)]
pub struct OrderUid(pub [u8; 56]);

impl Order {
    pub fn is_limit_order(&self) -> bool {
        matches!(self.class, Class::Limit)
    }

    /// For some orders the protocol doesn't precompute a fee. Instead solvers
    /// are supposed to compute a reasonable fee themselves.
    pub fn solver_determines_fee(&self) -> bool {
        self.is_limit_order()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Buy,
    Sell,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Class {
    Market,
    Limit,
    Liquidity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Interaction {
    pub target: H160,
    pub value: U256,
    pub call_data: Vec<u8>,
}

/// Source from which the sellAmount should be drawn upon order fulfillment
#[derive(Clone, Debug, PartialEq)]
pub enum SellTokenSource {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// Internal balances to the Vault with GPv2 relayer approval
    External,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BuyTokenDestination {
    /// Pay trade proceeds as an ERC20 token transfer
    Erc20,
    /// Pay trade proceeds as a Vault internal balance transfer
    Internal,
}

/// On the contract level orders have 32 bytes of generic data that are freely
/// choosable by the user. On the services level this is a hash of an app data
/// json document, which associates arbitrary information with an order while
/// being signed by the user.
#[derive(Clone, Debug, PartialEq)]
pub struct AppDataHash(pub [u8; 32]);

/// Signature over the order data.
/// All variants rely on the EIP-712 hash of the order data, referred to as the
/// order hash.
#[derive(Clone, Debug, PartialEq)]
pub enum Signature {
    /// The order struct is signed according to EIP-712.
    ///
    /// https://eips.ethereum.org/EIPS/eip-712
    Eip712(EcdsaSignature),
    /// The order hash is signed according to EIP-191's personal_sign signature
    /// format.
    ///
    /// https://eips.ethereum.org/EIPS/eip-191
    EthSign(EcdsaSignature),
    /// Signature verified according to EIP-1271, which facilitates a way for
    /// contracts to verify signatures using an arbitrary method. This
    /// allows smart contracts to sign and place orders. The order hash is
    /// passed to the verification method, along with this signature.
    ///
    /// https://eips.ethereum.org/EIPS/eip-1271
    Eip1271(Vec<u8>),
    /// For these signatures, the user broadcasts a transaction onchain. This
    /// transaction contains a signature of the order hash. Because this
    /// onchain transaction is also signed, it proves that the user indeed
    /// signed the order.
    PreSign,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EcdsaSignature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
}

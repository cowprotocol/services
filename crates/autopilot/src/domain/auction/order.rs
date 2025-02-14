use {
    crate::{
        domain,
        domain::{eth, fee},
    },
    primitive_types::{H160, H256, U256},
    std::fmt::{self, Debug, Display, Formatter},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub uid: OrderUid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub protocol_fees: Vec<fee::Policy>,
    pub side: Side,
    pub created: u32,
    pub valid_to: u32,
    pub receiver: Option<eth::Address>,
    pub owner: eth::Address,
    pub partially_fillable: bool,
    pub executed: TargetAmount,
    // Partially fillable orders should have their pre-interactions only executed
    // on the first fill.
    pub pre_interactions: Vec<Interaction>,
    pub post_interactions: Vec<Interaction>,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub app_data: AppDataHash,
    pub signature: Signature,
    pub quote: Option<domain::Quote>,
}

// uid as 56 bytes: 32 for orderDigest, 20 for ownerAddress and 4 for validTo
#[derive(Copy, Clone, PartialEq, Hash, Eq)]
pub struct OrderUid(pub [u8; 56]);

impl OrderUid {
    pub fn owner(&self) -> eth::Address {
        self.parts().1.into()
    }

    /// Splits an order UID into its parts.
    fn parts(&self) -> (H256, H160, u32) {
        (
            H256::from_slice(&self.0[0..32]),
            H160::from_slice(&self.0[32..52]),
            u32::from_le_bytes(self.0[52..].try_into().unwrap()),
        )
    }
}

impl Display for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes = [0u8; 2 + 56 * 2];
        bytes[..2].copy_from_slice(b"0x");
        // Unwrap because the length is always correct.
        hex::encode_to_slice(self.0.as_slice(), &mut bytes[2..]).unwrap();
        // Unwrap because the string is always valid utf8.
        let str = std::str::from_utf8(&bytes).unwrap();
        f.write_str(str)
    }
}

impl fmt::Debug for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Buy,
    Sell,
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
#[derive(Clone, derive_more::Debug, PartialEq)]
pub struct AppDataHash(#[debug("0x{}", hex::encode::<&[u8]>(self.0.as_ref()))] pub [u8; 32]);

/// Signature over the order data.
/// All variants rely on the EIP-712 hash of the order data, referred to as the
/// order hash.
#[derive(Clone, PartialEq)]
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

impl Signature {
    pub fn scheme(&self) -> SigningScheme {
        match self {
            Signature::Eip712(_) => SigningScheme::Eip712,
            Signature::EthSign(_) => SigningScheme::EthSign,
            Signature::Eip1271(_) => SigningScheme::Eip1271,
            Signature::PreSign => SigningScheme::PreSign,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Eip712(signature) | Self::EthSign(signature) => signature.to_bytes().to_vec(),
            Self::Eip1271(signature) => signature.clone(),
            Self::PreSign => Vec::new(),
        }
    }
}

impl Debug for Signature {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if let Signature::PreSign = self {
            return f.write_str("PreSign");
        }

        let scheme = format!("{:?}", self.scheme());
        let bytes = format!("0x{}", hex::encode(self.to_bytes()));
        f.debug_tuple(&scheme).field(&bytes).finish()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SigningScheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EcdsaSignature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
}

impl EcdsaSignature {
    /// r + s + v
    pub fn to_bytes(self) -> [u8; 65] {
        let mut bytes = [0u8; 65];
        bytes[..32].copy_from_slice(self.r.as_bytes());
        bytes[32..64].copy_from_slice(self.s.as_bytes());
        bytes[64] = self.v;
        bytes
    }
}

/// An amount denominated in the sell token for [`Side::Sell`] [`Order`]s, or in
/// the buy token for [`Side::Buy`] [`Order`]s.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetAmount(pub eth::U256);

impl From<eth::U256> for TargetAmount {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

impl From<TargetAmount> for eth::U256 {
    fn from(value: TargetAmount) -> Self {
        value.0
    }
}

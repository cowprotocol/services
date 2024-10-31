use {
    crate::{domain::eth, util::Bytes},
    model::signature::EcdsaSignature,
};

/// Signature over the order data.
#[derive(Debug, Clone)]
pub struct Signature {
    pub scheme: Scheme,
    pub data: Bytes<Vec<u8>>,
    /// The address used to sign and place this order.
    pub signer: eth::Address,
}

impl Signature {
    pub fn to_boundary_signature(&self) -> model::signature::Signature {
        // TODO Different signing schemes imply different sizes of signature data, which
        // indicates that I'm missing an invariant in my types and I need to fix
        // that PreSign, for example, carries no data. Everything should be
        // reflected in the types!
        match self.scheme {
            Scheme::Eip712 => model::signature::Signature::Eip712(EcdsaSignature::from_bytes(
                self.data.0.as_slice().try_into().unwrap(),
            )),
            Scheme::EthSign => model::signature::Signature::EthSign(EcdsaSignature::from_bytes(
                self.data.0.as_slice().try_into().unwrap(),
            )),
            Scheme::Eip1271 => model::signature::Signature::Eip1271(self.data.clone().into()),
            Scheme::PreSign => model::signature::Signature::PreSign,
        }
    }
}

/// The scheme used for signing the order. This is used by the solver and
/// the protocol, the driver does not care about the details of signature
/// verification.
#[derive(Debug, Clone, Copy)]
pub enum Scheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

pub fn domain_separator(
    chain_id: chain::Id,
    verifying_contract: eth::ContractAddress,
) -> eth::DomainSeparator {
    eth::DomainSeparator::new(&eth::DomainFields {
        type_hash:
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        name: b"Gnosis Protocol",
        version: b"v2",
        chain_id,
        verifying_contract,
    })
}

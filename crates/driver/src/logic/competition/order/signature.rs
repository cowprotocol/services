use crate::logic::eth;

/// Signature over the order data.
#[derive(Debug)]
pub struct Signature {
    pub data: Vec<u8>,
    pub scheme: Scheme,
}

/// The scheme used for signing the order. This is used by the solver and
/// the protocol, the driver does not care about the details of signature
/// verification.
#[derive(Debug)]
pub enum Scheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

pub fn domain_separator(
    chain_id: eth::ChainId,
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

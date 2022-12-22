//! Types and procedures defined by EIP-712.
//!
//! https://eips.ethereum.org/EIPS/eip-712

/// domainSeparator as defined by EIP-712.
///
/// https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
#[derive(Debug, Clone, Copy)]
pub struct DomainSeparator(pub [u8; 32]);

#[derive(Debug)]
pub struct DomainFields {
    pub type_hash: &'static [u8],
    pub name: &'static [u8],
    pub version: &'static [u8],
    pub chain_id: super::ChainId,
    pub verifying_contract: super::ContractAddress,
}

impl DomainSeparator {
    pub fn new(fields: &DomainFields) -> Self {
        let abi_string = ethabi::encode(&[
            ethabi::Token::Uint(web3::signing::keccak256(fields.type_hash).into()),
            ethabi::Token::Uint(web3::signing::keccak256(fields.name).into()),
            ethabi::Token::Uint(web3::signing::keccak256(fields.version).into()),
            ethabi::Token::Uint(fields.chain_id.0.into()),
            ethabi::Token::Address(fields.verifying_contract.into()),
        ]);
        Self(web3::signing::keccak256(abi_string.as_slice()))
    }
}

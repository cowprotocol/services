//! Types and procedures defined by EIP-712.
//!
//! https://eips.ethereum.org/EIPS/eip-712

/// domainSeparator as defined by EIP-712.
///
/// https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
#[derive(Debug, Clone, Copy)]
pub struct DomainSeparator(pub [u8; 32]);

impl DomainSeparator {
    pub fn new(chain_id: super::ChainId, verifying_contract: super::Contract) -> Self {
        let abi_string = ethabi::encode(&[
            ethabi::Token::Uint(web3::signing::keccak256(b"Gnosis Protocol").into()),
            ethabi::Token::Uint(web3::signing::keccak256(b"v2").into()),
            ethabi::Token::Uint(web3::signing::keccak256(
                b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
            ).into()),
            ethabi::Token::Uint(chain_id.0.into()),
            ethabi::Token::Address(verifying_contract.into()),
        ]);
        Self(web3::signing::keccak256(abi_string.as_slice()))
    }
}

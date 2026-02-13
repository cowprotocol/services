//! Types and procedures defined by EIP-712.
//!
//! https://eips.ethereum.org/EIPS/eip-712

/// domainSeparator as defined by EIP-712.
///
/// https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
#[derive(Debug, Clone, Copy)]
pub struct DomainSeparator(pub [u8; 32]);

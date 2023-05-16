//! Module implementing EIP-712 helpers.

use crate::DomainSeparator;

/// Computes the EIP-712 singing message by hashing the domain separator with a
/// hash of the structured data.
pub fn hash(domain: &DomainSeparator, struct_hash: &[u8; 32]) -> [u8; 32] {
    let mut buffer = [0u8; 66];
    buffer[0..2].copy_from_slice(&[0x19, 0x01]);
    buffer[2..34].copy_from_slice(&domain.0);
    buffer[34..66].copy_from_slice(struct_hash);
    web3::signing::keccak256(&buffer)
}

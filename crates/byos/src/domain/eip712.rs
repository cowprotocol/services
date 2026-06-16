use {
    alloy_primitives::{Address, B256, keccak256},
    alloy_sol_types::{Eip712Domain, SolStruct, sol},
};

sol! {
    /// EIP-712 typed data for BYOS proposals.
    #[derive(Default)]
    struct ProposalData {
        /// keccak256 of the 56-byte order UID
        bytes32 orderUidHash;
        uint256 sellAmount;
        uint256 buyAmount;
        uint256 validUntil;
        uint256 nonce;
    }
}

pub fn byos_domain(chain_id: u64) -> Eip712Domain {
    Eip712Domain {
        name: Some("BYOS".into()),
        version: Some("1".into()),
        chain_id: Some(alloy_primitives::U256::from(chain_id)),
        verifying_contract: None,
        salt: None,
    }
}

/// Recovers the signer address from a signed proposal.
pub fn recover_signer(
    proposal: &ProposalData,
    signature: &[u8; 65],
    domain: &Eip712Domain,
) -> Option<Address> {
    let signing_hash = proposal.eip712_signing_hash(domain);
    let sig = alloy_primitives::Signature::from_raw(signature).ok()?;
    sig.recover_address_from_prehash(&signing_hash).ok()
}

/// Computes the order UID hash used in the EIP-712 struct.
pub fn order_uid_hash(order_uid: &[u8; 56]) -> B256 {
    keccak256(order_uid)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::U256,
        alloy_signer::SignerSync,
        alloy_signer_local::PrivateKeySigner,
    };

    #[test]
    fn sign_and_recover_proposal() {
        let signer = PrivateKeySigner::random();
        let domain = byos_domain(1);

        let proposal = ProposalData {
            orderUidHash: B256::ZERO,
            sellAmount: U256::from(1000),
            buyAmount: U256::from(2000),
            validUntil: U256::from(u64::MAX),
            nonce: U256::ZERO,
        };

        let signing_hash = proposal.eip712_signing_hash(&domain);
        let sig = signer.sign_hash_sync(&signing_hash).unwrap();
        let sig_bytes: [u8; 65] = sig.as_bytes();

        let recovered = recover_signer(&proposal, &sig_bytes, &domain).unwrap();
        assert_eq!(recovered, signer.address());
    }
}

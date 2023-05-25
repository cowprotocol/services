//! App data refers to extra information that is associated with orders. This
//! information is not validated by the contract but it is used by other parts
//! of the system. For example, a user could specify that they want their order
//! to be COW only, which is something only the backend understands. Or what
//! their intended slippage when creating the order with the frontend was, which
//! adjusts the signed prices.
//!
//! On the smart contract level app data is freely choosable 32 bytes of signed
//! order data. This isn't enough space for some purposes so we interpret those
//! bytes as a hash of the full app data of arbitrary length. The full app data
//! is thus signed by the user when they sign the order.
//!
//! This crate specifies how the hash is calculated. It takes the keccak256 hash
//! of the input bytes. Additionally, it provides a canonical way to calculate
//! an IPFS CID from the hash. This allows full app data to be uploaded to IPFS.
//!
//! Note that not all app data hashes were created this way. As of 2023-05-25 we
//! are planning to move to the scheme implemented by this crate but orders have
//! been created with arbitrary app data hashes until now. See [this issue][0]
//! for more information.
//!
//! [0]: https://github.com/cowprotocol/services/issues/1465

use tiny_keccak::{Hasher, Keccak};

/// Hash full app data to get the bytes expected to be set as the contract level
/// app data.
pub fn hash_full_app_data(app_data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(app_data);
    let mut hash = [0u8; 32];
    hasher.finalize(&mut hash);
    hash
}

/// Create an IPFS CIDv1 from a hash created by `hash_full_app_data`.
///
/// The return value is the raw bytes of the CID. It is not multibase encoded.
pub fn create_ipfs_cid(app_data_hash: &[u8; 32]) -> [u8; 36] {
    let mut cid = [0u8; 4 + 32];
    cid[0] = 1; // cid version
    cid[1] = 0x55; // raw codec
    cid[2] = 0x1b; // keccak hash algorithm
    cid[3] = 32; // keccak hash length
    cid[4..].copy_from_slice(app_data_hash);
    cid
}

#[cfg(test)]
mod tests {
    use super::*;

    // Alternative way of calculating the expected values:
    // cat appdata | ipfs block put --mhtype keccak-256
    // -> bafkrwiek6tumtfzvo6yivqq5c7jtdkw6q3ar5pgfcjdujvrbzkbwl3eueq
    // ipfs cid format -b base16
    // bafkrwiek6tumtfzvo6yivqq5c7jtdkw6q3ar5pgfcjdujvrbzkbwl3eueq
    // -> f01551b208af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424
    // Remove the f prefix and you have the same CID.
    // Or check out the cid explorer:
    // - https://cid.ipfs.tech/#f01551b208af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424
    // - https://cid.ipfs.tech/#bafkrwiek6tumtfzvo6yivqq5c7jtdkw6q3ar5pgfcjdujvrbzkbwl3eueq
    #[test]
    fn known_good() {
        let full_app_data = r#"{"appCode":"CoW Swap","environment":"production","metadata":{"quote":{"slippageBips":"50","version":"0.2.0"},"orderClass":{"orderClass":"market","version":"0.1.0"}},"version":"0.6.0"}"#;
        let expected_hash =
            hex_literal::hex!("8af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424");
        let expected_cid = hex_literal::hex!(
            "01551b208af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424"
        );
        let hash = hash_full_app_data(full_app_data.as_bytes());
        let cid = create_ipfs_cid(&hash);
        assert_eq!(hash, expected_hash);
        assert_eq!(cid, expected_cid);
    }
}

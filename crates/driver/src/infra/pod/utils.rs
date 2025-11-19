use primitive_types::U256;

pub fn to_pod_u256(v: U256) -> pod_sdk::U256 {
    let mut bytes = [0u8; 32];
    v.to_big_endian(&mut bytes);
    pod_sdk::U256::from_be_bytes(bytes)
}

pub fn to_web3_h256(v: pod_sdk::U256) -> web3::types::H256 {
    let bytes: [u8; 32] = v.to_be_bytes();
    web3::types::H256::from_slice(&bytes)
}

pub fn u64_to_pod_timestamp(v: u64) -> pod_sdk::Timestamp {
    pod_sdk::Timestamp::from_micros(v as u128)
}

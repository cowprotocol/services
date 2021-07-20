//! Contains the app_data file structures to define additional data about tx origin

use crate::h160_hexadecimal::{self};
use anyhow::{anyhow, Result};
use cid::multihash::{Code, MultihashDigest};
use primitive_types::{H160, H256};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::convert::TryInto;

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct MetaData {
    #[serde(with = "h160_hexadecimal")]
    pub referrer: H160,
}

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct AppData {
    pub version: String,
    pub app_code: String,
    pub meta_data: MetaData,
}
impl AppData {
    pub fn sha_hash(&self) -> Result<H256> {
        let string = serde_json::to_string(self)?;
        // The following hash is the hash used by ipfs.
        // The ipfs cid can be calculated by
        // const RAW: u64 = 0x55;
        // let hash = Code::Sha2_256.digest(&string.into_bytes());
        // let cid = Cid::new_v1(RAW, hash);
        let hash = Code::Sha2_256.digest(&string.into_bytes());
        let array: [u8; 32] = hash.to_bytes()[2..]
            .try_into()
            .map_err(|_| anyhow!("h256 has wrong length"))?;
        Ok(H256::from(array))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn deserialization_and_back() {
        let value = json!(
        {
            "version": "1.0.0",
            "appCode": "CowSwap",
            "metaData": {
              "referrer": "0x424a46612794dbb8000194937834250dc723ffa5",
            }
        }
        );
        let expected = AppData {
            version: String::from("1.0.0"),
            app_code: String::from("CowSwap"),
            meta_data: MetaData {
                referrer: "0x424a46612794dbb8000194937834250dc723ffa5"
                    .parse()
                    .unwrap(),
            },
        };
        let deserialized: AppData = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(deserialized, expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }
    #[test]
    fn test_hash_calculation() {
        let app_data = AppData {
            version: String::from("1.5.0"),
            app_code: String::from("CowSwap"),
            meta_data: MetaData {
                referrer: "0x424a46612794dbb8000194937834250dc723ffa5"
                    .parse()
                    .unwrap(),
            },
        };
        let expected: H256 = "0x31e6e8bdeeb60ddb654e7b388102cebcea55ec31af639e0cf1c640f20579e47e"
            .parse()
            .unwrap();
        assert_eq!(app_data.sha_hash().unwrap(), expected);
    }
}

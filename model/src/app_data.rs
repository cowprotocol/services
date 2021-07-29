//! Contains the app_data file structures to define additional data about tx origin

use crate::h160_hexadecimal::{self};
use anyhow::{anyhow, Result};
use cid::multihash::{Code, MultihashDigest};
use primitive_types::{H160, H256};
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use serde_with::serde_as;
use std::convert::TryInto;

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize, Hash, Default)]
pub struct Referrer {
    #[serde(with = "h160_hexadecimal")]
    pub referrer: H160,
    pub version: String,
}

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize, Hash, Default)]
pub struct Metadata {
    pub referrer: Option<Referrer>,
}

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppData {
    pub version: String,
    pub app_code: Option<String>,
    pub metadata: Option<Metadata>,
}

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDataBlob(pub Value);

impl AppDataBlob {
    pub fn sha_hash(&self) -> Result<H256> {
        // The following hash is the hash used by ipfs.
        // The ipfs cid can be calculated by
        // const RAW: u64 = 0x55;
        // let hash = Code::Sha2_256.digest(&string.into_bytes());
        // let cid = Cid::new_v1(RAW, hash);
        // In order to avoid json duplication, we are deriving the hash from the json object
        let hash = Code::Sha2_256.digest(serde_json::ser::to_string(&self.0.clone())?.as_bytes());
        let array: [u8; 32] = hash.to_bytes()[2..]
            .try_into()
            .map_err(|_| anyhow!("h256 has wrong length"))?;
        Ok(H256::from(array))
    }
    pub fn get_app_data(&self) -> Result<AppData, serde_json::Error> {
        serde_json::from_value(self.0.clone())
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
            "appCode": "CowSwap",
            "version": "1.0.0",
            "metadata": {
              "referrer": {
                "referrer":  "0x424a46612794dbb8000194937834250dc723ffa5",
                "version": "0.3.4",
              }
            }
        }
        );
        let expected = AppData {
            version: String::from("1.0.0"),
            app_code: Some(String::from("CowSwap")),
            metadata: Some(Metadata {
                referrer: Some(Referrer {
                    referrer: "0x424a46612794dbb8000194937834250dc723ffa5"
                        .parse()
                        .unwrap(),
                    version: String::from("0.3.4"),
                }),
            }),
        };
        let deserialized = AppDataBlob(value.clone());
        assert_eq!(deserialized.get_app_data().unwrap(), expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }

    #[test]
    fn deserialization_and_back_for_nearly_empty_data() {
        let value = json!(
        {
            "appCode": serde_json::value::Value::Null,
            "version": "0.1",
            "metadata": serde_json::value::Value::Null
        }
        );
        let expected = AppData {
            app_code: None,
            version: String::from("0.1"),
            metadata: None,
        };
        let deserialized = AppDataBlob(value.clone());
        assert_eq!(deserialized.get_app_data().unwrap(), expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }
    #[test]
    fn test_hash_calculation() {
        let json = json!(
        {
            "appCode": "CowSwap",
            "version": "1.0.0",
            "metadata": {
              "referrer": {
                "referrer":  "0x424a46612794dbb8000194937834250dc723ffa5",
                "version": "0.3.4",
              }
            }
        }
        );
        let app_data_blob = AppDataBlob(json);
        let expected: H256 = "0x71b4723aecd9c98b339fba8d9e951534605d43ea43be12f43e068c3d11007af8"
            .parse()
            .unwrap();
        assert_eq!(app_data_blob.sha_hash().unwrap(), expected);
    }
}

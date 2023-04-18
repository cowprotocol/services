//! Code for interacting with Tenderly during test setup.

use {
    ethcontract::H160,
    std::{collections::HashMap, path::Path},
};

#[derive(Debug)]
pub struct Tenderly {
    pub username: String,
    pub project: String,
    pub key: String,
}

/// Create a Tenderly fork starting from the mainnet genesis block.
pub async fn fork(tenderly: Tenderly) -> Fork {
    let client = reqwest::Client::new();
    // Create a new fork.
    let id = client
        .post(format!(
            "https://api.tenderly.co/api/v1/account/{}/project/{}/fork",
            tenderly.username, tenderly.project
        ))
        .header("x-access-key", &tenderly.key)
        .json(&serde_json::json!({
            "network_id": "1",
            "block_number": 0,
            "alias": "the one",
            "description": "",
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()
        .get("simulation_fork")
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    Fork { id, tenderly }
}

pub struct Fork {
    id: String,
    tenderly: Tenderly,
}

impl Fork {
    pub fn web3_url(&self) -> String {
        format!("https://rpc.tenderly.co/fork/{}", self.id)
    }

    /// Verify a contract on the fork. This uploads the source code of the
    /// contract to Tenderly and allows Tenderly to provide high-quality
    /// debugging support.
    pub async fn verify(&self, addr: H160, source: contracts::Source) {
        let client = reqwest::Client::new();
        let mut dir = read_dir(&source.dir, &source.dir);
        // Ensure that the main file is first.
        dir.sort_by(|file, _| {
            if file == source.file {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        });
        let req = VerifyRequest {
            config: VerifyConfig {
                optimizations_count: source.optimization_runs,
                optimizations_used: source.optimizations,
                compiler_version: source.compiler_version.to_owned(),
            },
            contracts: dir
                .into_iter()
                .map(|file| Contract {
                    contract_name: if file == source.file {
                        Some(file.clone())
                    } else {
                        None
                    },
                    networks: if file == source.file {
                        Some(
                            [(
                                self.id.clone(),
                                Network {
                                    address: format!("0x{}", hex::encode(addr.as_bytes())),
                                },
                            )]
                            .into_iter()
                            .collect(),
                        )
                    } else {
                        None
                    },
                    source: std::fs::read_to_string(source.dir.join(file.clone())).unwrap(),
                    source_path: file,
                })
                .collect(),
        };
        let resp = client
            .post(format!(
                "https://api.tenderly.co/api/v1/account/{}/project/{}/fork/{}/verify",
                self.tenderly.username, self.tenderly.project, self.id
            ))
            .header("x-access-key", &self.tenderly.key)
            .json(&req)
            .send()
            .await
            .unwrap();
        let status = resp.status();
        assert_eq!(status, 200);
    }
}

fn read_dir(base: &Path, dir: &Path) -> Vec<String> {
    let mut result = Vec::new();
    for file in std::fs::read_dir(dir).unwrap().map(|e| e.unwrap()) {
        if file.file_type().unwrap().is_dir() {
            result.extend(read_dir(base, &file.path()));
        } else {
            result.push(
                file.path()
                    .to_str()
                    .unwrap()
                    .to_owned()
                    .trim_start_matches(base.to_str().unwrap())
                    .trim_start_matches("/")
                    .to_owned(),
            );
        }
    }
    result
}

#[derive(Debug, serde::Serialize)]
struct VerifyRequest {
    config: VerifyConfig,
    contracts: Vec<Contract>,
}

#[derive(Debug, serde::Serialize)]
struct VerifyConfig {
    optimizations_count: u32,
    optimizations_used: bool,
    compiler_version: String,
}

#[derive(Debug, serde::Serialize)]
struct Contract {
    #[serde(rename = "contractName")]
    contract_name: Option<String>,
    networks: Option<HashMap<String, Network>>,
    source: String,
    #[serde(rename = "sourcePath")]
    source_path: String,
}

#[derive(Debug, serde::Serialize)]
struct Network {
    address: String,
}

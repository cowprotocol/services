use {
    simple_logger::SimpleLogger,
    std::{collections::HashMap, path::Path},
    web3::{transports::Http, Web3},
};

const USERNAME: &'static str = "gp-v2";
const PROJECT: &'static str = "niksa-";
const API_KEY: &'static str = "LU0ldRKOdvSKsooOFYGdEOK6Vpy2AFOL";

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    log::debug!("starting");

    let client = reqwest::Client::new();
    // Create a new fork.
    let fork = client
        .post(format!(
            "https://api.tenderly.co/api/v1/account/{USERNAME}/project/{PROJECT}/fork"
        ))
        .header("x-access-key", API_KEY)
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
    let rpc = format!("https://rpc.tenderly.co/fork/{fork}");
    let web3 = Web3::new(Http::new(&rpc).unwrap());
    let address = web3.eth().accounts().await.unwrap()[0];
    let contract = contracts::BalancerV2Authorizer::builder(&web3, address)
        .from(contracts::ethcontract::Account::Local(address, None))
        .gas(25254000000u128.into())
        .deploy()
        .await
        .unwrap();

    let balancer = contracts::source::balancer_v2_authorizer();
    let mut dir = read_dir(&balancer.dir, &balancer.dir);
    // Ensure that the main file is first.
    dir.sort_by(|file, _| {
        if file == balancer.file {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    });
    let body = Body {
        config: Config {
            optimizations_count: balancer.optimization_runs,
            optimizations_used: balancer.optimizations,
            compiler_version: balancer.compiler_version.to_owned(),
        },
        contracts: dir
            .into_iter()
            .map(|file| Contract {
                contract_name: if file == balancer.file {
                    Some(file.clone())
                } else {
                    None
                },
                networks: if file == balancer.file {
                    Some(
                        [(
                            fork.clone(),
                            Network {
                                address: format!(
                                    "0x{}",
                                    hex::encode(contract.address().as_bytes())
                                ),
                            },
                        )]
                        .into_iter()
                        .collect(),
                    )
                } else {
                    None
                },
                source: std::fs::read_to_string(balancer.dir.join(file.clone())).unwrap(),
                source_path: file,
            })
            .collect(),
    };
    dbg!(&body);
    let verification = client
        .post(format!(
            "https://api.tenderly.co/api/v1/account/{USERNAME}/project/{PROJECT}/fork/{fork}/verify"
        ))
        .header("x-access-key", API_KEY)
        .json(&body)
        .send()
        .await
        .unwrap();

    dbg!(verification.status());
    dbg!(verification.text().await.unwrap());
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
struct Body {
    config: Config,
    contracts: Vec<Contract>,
}

#[derive(Debug, serde::Serialize)]
struct Config {
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

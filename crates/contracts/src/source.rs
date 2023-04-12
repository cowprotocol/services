use std::path::PathBuf;

#[derive(Debug)]
pub struct Source {
    pub dir: PathBuf,
    pub file: &'static str,
    pub name: &'static str,
    pub compiler_version: &'static str,
    // TODO Check if this is needed.
    pub compiler_commit: &'static str,
    pub optimizations: bool,
    pub optimization_runs: u32,
}

pub fn weth9() -> Source {
    Source {
        dir: path("."),
        file: "WETH9.sol",
        name: "WETH9",
        compiler_version: "v0.4.19",
        compiler_commit: "",
        optimizations: false,
        optimization_runs: 200,
    }
}

pub fn balancer_v2_authorizer() -> Source {
    Source {
        dir: path("balancer"),
        file: "TimelockAuthorizer.sol",
        name: "TimelockAuthorizer",
        compiler_version: "v0.7.1",
        compiler_commit: "",
        optimizations: true,
        optimization_runs: 9999,
    }
}

fn path(p: &str) -> PathBuf {
    let file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    file.join("artifacts/source").join(p)
}

/*
pub const BALANCER_V2_AUTHORIZER: Source = Source {
    name: "BalancerV2Authorizer",
    path: "BalancerV2Authorizer.sol",
    code: include_str!("../artifacts/BalancerV2Authorizer.sol"),
    compiler: "v0.6.6+commit.6c089d02",
    optimizations: true,
    optimization_runs: 200,
    evm: None,
};
*/

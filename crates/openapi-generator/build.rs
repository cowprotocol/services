// Trick to generate the OpenAPI spec on build time.
// See: https://github.com/juhaku/utoipa/issues/214#issuecomment-1179589373

use std::fs;

const SOLVERS_OPENAPI_PATH: &str = "../solvers/openapi.json";

fn main() {
    let openapi_yaml = solvers::generate_openapi_json()
        .expect("Error generating the solvers OpenAPI documentation");
    fs::write(SOLVERS_OPENAPI_PATH, openapi_yaml)
        .expect("Error writing the solvers OpenAPI documentation");
}

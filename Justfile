help:
    @just --list

# Run unit tests
test-unit:
    cargo nextest run --locked

# Run doc tests
test-doc:
    cargo test --doc --locked

# Run database tests
test-db:
    cargo nextest run postgres --test-threads 1 --run-ignored ignored-only --locked

# Run End-to-end tests on local node (this machine)
test-e2e-local: (test-e2e "local_node")

# Run End-to-end tests on forked node
#
# FORK_URL_MAINNET and FORK_URL_GNOSIS env variables have to be provided.
test-e2e-forked: (test-e2e "forked_node")

# Run End-to-end tests with custom filters
test-e2e *filters:
    cargo nextest run -p e2e {{filters}} --test-threads 1 --failure-output final --run-ignored ignored-only --locked

test-driver:
    RUST_MIN_STACK=3145728 cargo nextest run -p driver --test-threads 1 --run-ignored ignored-only --locked

# Run clippy
clippy:
    cargo clippy --locked --workspace --all-features --all-targets -- -D warnings

# Format the repository
fmt *extra:
    cargo +nightly fmt --all -- {{extra}}

# Start database for E2E tests
start-db:
    docker compose up -d

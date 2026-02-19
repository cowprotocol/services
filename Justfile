help:
    @just --list

# Run unit tests
test-unit:
    cargo nextest run

# Run doc tests
test-doc:
    cargo test --doc

# Run database tests
test-db:
    cargo nextest run postgres --test-threads 1 --run-ignored ignored-only

# Run End-to-end tests on local node (this machine)
test-e2e-local: (test-e2e "local_node")

# Run End-to-end tests on forked node
#
# FORK_URL_MAINNET and FORK_URL_GNOSIS env variables have to be provided.
test-e2e-forked: (test-e2e "forked_node")

# Run End-to-end tests with custom filters
test-e2e filters="" *extra="":
    cargo nextest run -p e2e '{{filters}}' --test-threads 1 --failure-output final --run-ignored ignored-only {{extra}}

test-driver:
    RUST_MIN_STACK=3145728 cargo nextest run -p driver --test-threads 1 --run-ignored ignored-only

# Run clippy
clippy:
    cargo clippy --locked --workspace --all-features --all-targets -- -D warnings

# Format the repository
fmt *extra:
    cargo +nightly fmt --all -- {{extra}}

# Start database for E2E tests
start-db:
    docker compose up -d

# Properly formats all ABI files that we generate bindings for to make them human readable
format-abi-files:
    #!/bin/sh
    cd ./crates/contracts/artifacts
    for f in *.json; do
        if [ -L "$f" ]; then
            echo "Skipping symlink: $f"
            continue
        fi
        jq . "$f" > "$f.tmp" && mv "$f.tmp" "$f"
    done

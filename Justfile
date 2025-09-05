test:
    cargo nextest run

test-db:
    cargo nextest run postgres --test-threads 1 --run-ignored ignored-only

test-e2e-local:
    cargo nextest run -p e2e local_node --test-threads 1 --failure-output final --run-ignored ignored-only

test-e2e-forked fork_url:
    FORK_URL={{fork_url}} cargo nextest run -p e2e forked_node --test-threads 1 --run-ignored ignored-only --failure-output final

clippy:
    cargo clippy --all-features --all-targets -- -D warnings

fmt:
    cargo +nightly fmt --all

start-db:
    docker compose up

start-anvil:
    ANVIL_IP_ADDR=0.0.0.0 anvil \
        --gas-price 1 \
        --gas-limit 10000000 \
        --base-fee 0 \
        --balance 1000000 \
        --chain-id 1 \
        --timestamp 1577836800

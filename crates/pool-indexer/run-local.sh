#!/usr/bin/env bash
# Wipes the local test DB, re-applies migrations, and runs pool-indexer
# against config.local.toml (mainnet + Ink by default).
set -euo pipefail

cd "$(dirname "$0")/../.."

CONFIG="crates/pool-indexer/config.local.toml"

if [[ ! -f "$CONFIG" ]]; then
  echo "missing $CONFIG — copy/edit config.local.toml first" >&2
  exit 1
fi

echo "==> tearing down docker compose (with volumes)"
docker compose down --volumes

echo "==> starting postgres"
docker compose up -d db

echo "==> waiting for postgres to accept connections"
until docker compose exec -T db pg_isready -U "$USER" >/dev/null 2>&1; do
  sleep 1
done

echo "==> running flyway migrations"
docker compose run --rm migrations

echo "==> starting pool-indexer"
export RUST_LOG=info,pool_indexer=debug 
exec cargo run --release -p pool-indexer -- --config "$CONFIG"

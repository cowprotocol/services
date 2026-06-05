#!/usr/bin/env bash
#
# Provisions the one thing the e2e test suite needs but the base image can't
# provide out of the box:
#
#   * Postgres server – we run a native server (rather than the docker-compose
#              one) so it survives container restarts and is up before any test
#              runs, then apply the flyway migrations the harness expects to
#              already exist.
#
# anvil/forge come from the foundry devcontainer feature; their prebuilt binaries
# run directly on the bookworm base image (glibc >= 2.32), so nothing to do here.
#
# Runs as `postCreateCommand` (once, when the container is created).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# -----------------------------------------------------------------------------
echo "==> postgres"
# Install a server only if no cluster exists yet (fresh container). Reuse whatever
# version is already present otherwise, so this is safe to re-run.
if ! ls -d /etc/postgresql/*/main >/dev/null 2>&1; then
    echo "    installing postgresql-16..."
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq postgresql-16
fi
PG_VERSION="$(ls /etc/postgresql | sort -n | tail -1)"
echo "    using cluster ${PG_VERSION}/main"

# Start the cluster (shared with postStart so the two can't drift).
bash "$REPO_ROOT/.devcontainer/start-postgres.sh"

# Trust auth for local connections (development container only).
HBA="/etc/postgresql/${PG_VERSION}/main/pg_hba.conf"
sudo tee "$HBA" >/dev/null <<'EOF'
local   all   all                  trust
host    all   all   127.0.0.1/32   trust
host    all   all   ::1/128        trust
EOF
sudo pg_ctlcluster "$PG_VERSION" main reload

# The e2e harness connects with the bare url `postgresql://`, which resolves to a
# role and database named after $PGUSER/$PGDATABASE (see containerEnv in
# devcontainer.json and DatabasePoolConfig::test_default).
psql -h 127.0.0.1 -U postgres -d postgres -tAc \
    "SELECT 1 FROM pg_roles WHERE rolname='${PGUSER}'" | grep -q 1 \
    || psql -h 127.0.0.1 -U postgres -d postgres -c \
        "CREATE ROLE \"${PGUSER}\" LOGIN SUPERUSER;"
psql -h 127.0.0.1 -U postgres -d postgres -tAc \
    "SELECT 1 FROM pg_database WHERE datname='${PGDATABASE}'" | grep -q 1 \
    || psql -h 127.0.0.1 -U postgres -d postgres -c \
        "CREATE DATABASE \"${PGDATABASE}\" OWNER \"${PGUSER}\";"

# Apply the migrations with the same flyway image the rest of the repo uses
# (see docker-compose.yaml). Flyway tracks what it has already applied, so this is
# incremental and idempotent: re-running only applies new migrations and fails
# loudly if one is incompatible with the current schema. The native server runs
# on the host network namespace, so `--network=host` lets flyway reach it on
# 127.0.0.1:${PGPORT}.
echo "    applying flyway migrations..."
docker run --rm --network=host \
    -v "$REPO_ROOT/database/sql:/flyway/sql:ro" \
    -v "$REPO_ROOT/database/conf:/flyway/conf:ro" \
    flyway/flyway:10.7.1 \
    -url="jdbc:postgresql://127.0.0.1:${PGPORT}/${PGDATABASE}?user=${PGUSER}&password=" \
    migrate

echo "==> e2e environment ready (postgres)"

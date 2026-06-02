#!/usr/bin/env bash
#
# Provisions the one thing the e2e test suite needs but the base image can't
# provide out of the box:
#
#   * Postgres server – the Docker daemon can't start in this container, so the
#              docker-compose Postgres isn't available. We run a native server and
#              apply the flyway migrations the harness expects to already exist.
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

# Start the cluster (there is no init system to do it for us). `start` errors if
# the cluster is already running, so only start it when it isn't online; a
# genuine start failure then surfaces instead of being swallowed.
if ! pg_lsclusters -h "$PG_VERSION" main | grep -q online; then
    sudo pg_ctlcluster "$PG_VERSION" main start
fi

# Trust auth for local connections (development container only).
HBA="/etc/postgresql/${PG_VERSION}/main/pg_hba.conf"
sudo tee "$HBA" >/dev/null <<'EOF'
local   all   all                  trust
host    all   all   127.0.0.1/32   trust
host    all   all   ::1/128        trust
EOF
sudo pg_ctlcluster "$PG_VERSION" main reload

# The e2e harness connects with the bare url `postgresql://`, which resolves to a
# role and database named after the OS user (see containerEnv in devcontainer.json
# and DatabasePoolConfig::test_default).
psql -h 127.0.0.1 -U postgres -d postgres -tAc \
    "SELECT 1 FROM pg_roles WHERE rolname='${USER}'" | grep -q 1 \
    || psql -h 127.0.0.1 -U postgres -d postgres -c \
        "CREATE ROLE \"${USER}\" LOGIN SUPERUSER;"
psql -h 127.0.0.1 -U postgres -d postgres -tAc \
    "SELECT 1 FROM pg_database WHERE datname='${USER}'" | grep -q 1 \
    || psql -h 127.0.0.1 -U postgres -d postgres -c \
        "CREATE DATABASE \"${USER}\" OWNER \"${USER}\";"

# Apply the flyway migrations once (the harness only truncates tables, it does not
# create the schema). Skip if the schema is already present.
if psql -h 127.0.0.1 -U "$USER" -d "$USER" -tAc \
        "SELECT to_regclass('public.orders')" | grep -q orders; then
    echo "    schema already present, skipping migrations"
else
    echo "    applying $(ls "$REPO_ROOT"/database/sql/V*.sql | wc -l) migrations..."
    for f in $(ls "$REPO_ROOT"/database/sql/V*.sql | sort); do
        psql -v ON_ERROR_STOP=1 -q -h 127.0.0.1 -U "$USER" -d "$USER" -f "$f"
    done
fi

echo "==> e2e environment ready (postgres)"

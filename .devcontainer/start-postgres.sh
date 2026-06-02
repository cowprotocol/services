#!/usr/bin/env bash
#
# Starts the local Postgres cluster on every container start. There is no init
# system in the container, so the cluster does not come up on its own after a
# stop/start. Provisioning (install, role, db, migrations) is done once by
# setup-e2e.sh; this only (re)starts what already exists.
#
# Runs as `postStartCommand`.
set -euo pipefail

PG_VERSION="$(ls /etc/postgresql 2>/dev/null | sort -n | tail -1 || true)"
if [ -n "$PG_VERSION" ]; then
    sudo pg_ctlcluster "$PG_VERSION" main start 2>/dev/null || true
fi

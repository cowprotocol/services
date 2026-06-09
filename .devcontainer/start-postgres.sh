#!/usr/bin/env bash
#
# Starts the local Postgres cluster on every container start. There is no init
# system in the container, so the cluster does not come up on its own after a
# stop/start. Provisioning (install, role, db, migrations) is done once by
# setup-e2e.sh; this only (re)starts what already exists.
#
# Runs as `postStartCommand`.
set -euo pipefail

PG_VERSION="$(ls /etc/postgresql | sort -n | tail -1)"

# `start` errors if the cluster is already running, so only start it when it
# isn't online; a genuine start failure then surfaces instead of being swallowed.
if ! pg_lsclusters -h "$PG_VERSION" main | grep -q online; then
    sudo pg_ctlcluster "$PG_VERSION" main start
fi

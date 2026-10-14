"""Prints which slow CI test jobs the changes from BASE to HEAD affect.

Usage: python3 affected-jobs.py BASE HEAD

Prints one `job=true|false` line per job. A job runs when it builds a crate
that owns a changed file or whose Cargo.lock entry changed, or when a changed
file matches one of its extra globs. A changed file outside every crate and
glob runs every job.
"""

import fnmatch
import json
import os
import subprocess
import sys
import tomllib

# The crates whose tests each job runs. `solana` names every Solana service,
# since its `solana_db` filter can match tests in any of them.
ROOTS = {
    "driver": ["driver"],
    "e2e": ["e2e"],
    "solana": [
        "autopilot-svm",
        "solana-driver",
        "solana-indexer",
        "solana-orderbook",
        "solana-solvers",
    ],
}
# Files outside every crate that a job reads.
EXTRA = {
    "e2e": ["database/conf/*", "database/sql/*", "database/sql-pool-indexer/*"],
    "solana": ["database/conf/*", "database/sql-solana/*"],
}
# Files outside every crate that no job reads.
IGNORED = ["*.md", "docs/*", "playground/*"]


def git(*args):
    return subprocess.check_output(["git", *args], text=True)


def lock(rev):
    return tomllib.loads(git("show", f"{rev}:Cargo.lock"))["package"]


def matches(path, globs):
    return any(fnmatch.fnmatch(path, glob) for glob in globs)


base, head = sys.argv[1:]
metadata = json.loads(
    subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"])
)
owners = {
    os.path.relpath(os.path.dirname(p["manifest_path"]), metadata["workspace_root"])
    + "/": p["name"]
    for p in metadata["packages"]
}

# Dependencies by crate name across both lockfiles, so removed edges count too.
old, new = lock(base), lock(head)
deps = {}
for crate in old + new:
    deps.setdefault(crate["name"], set()).update(
        dep.split()[0] for dep in crate.get("dependencies", [])
    )
assert all(root in deps for roots in ROOTS.values() for root in roots), (
    "a ROOTS crate is missing from Cargo.lock"
)

entries = [{json.dumps(c, sort_keys=True) for c in crates} for crates in (old, new)]
changed = {json.loads(entry)["name"] for entry in entries[0] ^ entries[1]}
run = dict.fromkeys(ROOTS, False)
for path in git("diff", "--name-only", "--no-renames", base, head).splitlines():
    owner = max((d for d in owners if path.startswith(d)), key=len, default=None)
    if owner:
        changed.add(owners[owner])
    elif path != "Cargo.lock" and not matches(path, IGNORED):
        hits = [job for job, globs in EXTRA.items() if matches(path, globs)]
        for job in hits or ROOTS:
            run[job] = True

for job, roots in ROOTS.items():
    builds, stack = set(), list(roots)
    while stack:
        crate = stack.pop()
        if crate not in builds:
            builds.add(crate)
            stack += deps.get(crate, ())
    print(f"{job}={str(run[job] or bool(builds & changed)).lower()}")

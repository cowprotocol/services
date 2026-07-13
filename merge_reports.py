#!/usr/bin/env python3
"""Merge settlement-finder report JSONs by the environment they were collected
from, producing one file per (environment, network) — e.g. `staging-ink.json`,
`prod-ink.json`.

Both the environment and the network come from the `verify_runs` table in the
settlement-finder progress SQLite database, which records the `db_url` and
`network` behind each `report_path`:

  * a read-replica RDS host  -> "staging"
  * localhost                -> "prod"

Files present on disk but absent from the DB have no known environment; they are
grouped under "unknown" using the `network` field inside the JSON, and flagged.

This script is non-destructive: it only reads the source reports and the SQLite
file, and writes merged output to a new directory.
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

Report = dict[str, Any]
Mismatch = dict[str, Any]

UNKNOWN_ENV = "unknown"


@dataclass(frozen=True)
class Target:
    """The (environment, network) bucket a report is merged into."""

    environment: str
    network: str

    @property
    def slug(self) -> str:
        return f"{self.environment}-{self.network}"


@dataclass
class VerifyRun:
    """One row of the `verify_runs` table that produced a report file."""

    run_id: int
    created_at: str
    from_block: int
    to_block: int
    mismatches: int
    truncated: bool


@dataclass
class DbEntry:
    """What the `verify_runs` table records about one report file."""

    target: Target
    runs: list[VerifyRun] = field(default_factory=list)


@dataclass
class Source:
    """A single report file, resolved to the bucket it belongs to."""

    name: str
    report: Report
    verify_runs: list[VerifyRun] = field(default_factory=list)

    @property
    def in_sqlite(self) -> bool:
        return bool(self.verify_runs)

    @property
    def mismatches(self) -> list[Mismatch]:
        return self.report.get("mismatches") or []


def environment_of(db_url: str) -> str:
    """Classify a run's database URL as "prod" (localhost) or "staging" (remote)."""
    host = urlsplit(db_url).hostname or ""
    return "prod" if host in ("localhost", "127.0.0.1") else "staging"


def load_db_mapping(sqlite_path: Path) -> dict[str, DbEntry]:
    """Map each report file's basename to its target bucket and producing runs."""
    mapping: dict[str, DbEntry] = {}
    with sqlite3.connect(f"file:{sqlite_path}?mode=ro", uri=True) as conn:
        conn.row_factory = sqlite3.Row
        rows = conn.execute(
            "SELECT id, network, db_url, from_block, to_block, mismatches, "
            "truncated, report_path, created_at "
            "FROM verify_runs WHERE report_path IS NOT NULL"
        )
        for row in rows:
            target = Target(environment_of(row["db_url"]), row["network"])
            entry = mapping.setdefault(Path(row["report_path"]).name, DbEntry(target))
            entry.runs.append(
                VerifyRun(
                    run_id=row["id"],
                    created_at=row["created_at"],
                    from_block=row["from_block"],
                    to_block=row["to_block"],
                    mismatches=row["mismatches"],
                    truncated=bool(row["truncated"]),
                )
            )
    return mapping


def collect_sources(
    reports_dir: Path, db_mapping: dict[str, DbEntry]
) -> tuple[dict[Target, list[Source]], list[str]]:
    """Group every report file under its (environment, network) target.

    The target comes from SQLite when the file is recorded there. Files absent
    from the DB have an unknown environment; they fall back to their internal
    `network` field and are returned as the second element so the caller can flag
    them.
    """
    grouped: dict[Target, list[Source]] = {}
    unmapped: list[str] = []

    for path in sorted(reports_dir.glob("*.json")):
        report = json.loads(path.read_text())
        entry = db_mapping.get(path.name)

        if entry:
            target, runs = entry.target, entry.runs
        else:
            network = report.get("network")
            target, runs = Target(UNKNOWN_ENV, network), []
            unmapped.append(path.name)
            if not network:
                print(
                    f"WARN: {path.name} not in sqlite and has no internal "
                    "network; skipping",
                    file=sys.stderr,
                )
                continue

        grouped.setdefault(target, []).append(Source(path.name, report, runs))

    return grouped, unmapped


def dedupe_mismatches(sources: list[Source]) -> tuple[list[Mismatch], dict[str, dict]]:
    """Concatenate mismatches across sources, dropping ones already seen.

    Overlapping block ranges across runs mean the same mismatch can appear in
    several files; identity is the tuple below. Also returns per-file counts of
    how many mismatches each source contributed vs. duplicated.
    """
    seen: set[tuple] = set()
    merged: list[Mismatch] = []
    stats: dict[str, dict] = {}

    for source in sources:
        added = duplicate = 0
        for mismatch in source.mismatches:
            key = mismatch_key(mismatch)
            if key in seen:
                duplicate += 1
                continue
            seen.add(key)
            merged.append(mismatch)
            added += 1
        stats[source.name] = {"added": added, "duplicate": duplicate}

    merged.sort(key=lambda m: (m.get("block") or 0, m.get("log_index") or 0))
    return merged, stats


def mismatch_key(mismatch: Mismatch) -> tuple:
    """Stable identity for a mismatch so overlapping block ranges don't double-count."""
    return (
        mismatch.get("block"),
        mismatch.get("chain_tx_hash"),
        mismatch.get("kind"),
        mismatch.get("log_index"),
        mismatch.get("order_uid"),
        mismatch.get("solver"),
    )


def summarize(mismatches: list[Mismatch]) -> dict:
    """Recompute the report summary from the merged mismatch list."""
    by_kind = Counter(m.get("kind", "unknown") for m in mismatches)
    blocks = {m.get("block") for m in mismatches}
    return {
        "by_kind": dict(sorted(by_kind.items())),
        "mismatch_blocks": len(blocks),
        "total": len(mismatches),
    }


def _bound(reports: list[Report], key: str, select) -> Any:
    """min/max of `key` across reports, ignoring missing values."""
    values = [r[key] for r in reports if r.get(key) is not None]
    return select(values) if values else None


def _union(reports: list[Report], key: str) -> list:
    """Sorted union of a list-valued (or scalar) field across reports."""
    values: set = set()
    for report in reports:
        field_value = report.get(key)
        if isinstance(field_value, list):
            values.update(field_value)
        elif field_value:
            values.add(field_value)
    return sorted(values)


def merge_target(target: Target, sources: list[Source]) -> Report:
    """Merge all report files for one (environment, network) target."""
    sources = sorted(sources, key=lambda s: s.name)
    reports = [s.report for s in sources]

    mismatches, stats = dedupe_mismatches(sources)
    chain_ids = _union(reports, "chain_id")

    return {
        "environment": target.environment,
        "network": target.network,
        "chain_id": chain_ids[0] if len(chain_ids) == 1 else chain_ids,
        "contracts": _union(reports, "contracts"),
        "rpc": _union(reports, "rpc"),
        "from": _bound(reports, "from", min),
        "to": _bound(reports, "to", max),
        "scanned_through": _bound(reports, "scanned_through", max),
        "blocks_scanned": sum(r.get("blocks_scanned") or 0 for r in reports),
        "truncated": any(r.get("truncated") for r in reports),
        "summary": summarize(mismatches),
        "source_reports": [describe_source(s, stats[s.name]) for s in sources],
        "mismatches": mismatches,
    }


def describe_source(source: Source, stats: dict) -> dict:
    """Provenance entry for one contributing file."""
    report = source.report
    return {
        "file": source.name,
        "in_sqlite": source.in_sqlite,
        "chain_id": report.get("chain_id"),
        "from": report.get("from"),
        "to": report.get("to"),
        "scanned_through": report.get("scanned_through"),
        "truncated": bool(report.get("truncated")),
        "mismatches_in_file": len(source.mismatches),
        "mismatches_added": stats["added"],
        "mismatches_duplicate": stats["duplicate"],
        "verify_runs": [vars(run) for run in source.verify_runs],
    }


def write_json(path: Path, data: Any) -> None:
    path.write_text(json.dumps(data, indent=2) + "\n")


def write_outputs(out_dir: Path, grouped: dict[Target, list[Source]]) -> list[dict]:
    """Write one merged report per target plus an index; return the index."""
    out_dir.mkdir(parents=True, exist_ok=True)
    index: list[dict] = []

    for target in sorted(grouped, key=lambda t: t.slug):
        sources = grouped[target]
        merged = merge_target(target, sources)
        write_json(out_dir / f"{target.slug}.json", merged)

        summary = merged["summary"]
        index.append(
            {
                "environment": target.environment,
                "network": target.network,
                "output": f"{target.slug}.json",
                "source_files": len(sources),
                "total_mismatches": summary["total"],
                "mismatch_blocks": summary["mismatch_blocks"],
            }
        )
        print(
            f"{target.slug:22s} <- {len(sources):2d} files "
            f"-> {summary['total']:7d} mismatches "
            f"({summary['mismatch_blocks']} blocks)"
        )

    write_json(out_dir / "index.json", index)
    return index


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reports-dir", type=Path, default=Path("reports"))
    parser.add_argument(
        "--sqlite", type=Path, default=Path("settlement-finder-progress.sqlite")
    )
    parser.add_argument("--out-dir", type=Path, default=Path("reports-merged"))
    return parser.parse_args()


def main() -> None:
    args = parse_args()

    db_mapping = load_db_mapping(args.sqlite)
    grouped, unmapped = collect_sources(args.reports_dir, db_mapping)
    index = write_outputs(args.out_dir, grouped)

    print(f"\nWrote {len(index)} merged reports to {args.out_dir}/")
    if unmapped:
        print(
            f"Note: {len(unmapped)} file(s) not in sqlite, grouped under "
            f"'{UNKNOWN_ENV}' by internal network field: {', '.join(unmapped)}"
        )


if __name__ == "__main__":
    main()

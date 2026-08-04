#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["matplotlib>=3.8"]
# ///
"""Charts a `multicall-bench run --csv` file.

    ./plot.py output.csv multicall-bench.png

Dependencies are declared inline, so `uv run` fetches them into a throwaway
environment and there is nothing to install.

The unbatched path is drawn as a reference band rather than as another position
on the x axis: it is the baseline the multicall sizes are measured against, not
"batch size zero", and sharing an axis with them reads as one continuum and
hides the comparison the chart exists to make.
"""

import argparse
import csv
import statistics
import sys
from collections import defaultdict
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D

# Slots 1-3 of the reference categorical palette, which pass the all-pairs CVD
# gate. Hue carries the ethrpc batch size; the dash pattern and marker carry the
# concurrency, so no series is told apart by colour alone.
HUES = ["#2a78d6", "#eb6834", "#1baf7a"]
STYLES = [("solid", "o"), ((0, (5, 2)), "s")]

SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK_2 = "#52514e"
MUTED = "#898781"
GRIDLINE = "#e1e0d9"
BASELINE = "#c3c2b7"
BAND = "#eeede9"

# The unbatched baseline. Everything else on the x axis is a multicall size.
UNBATCHED = 0

REQUIRED = [
    "multicall_batch_size",
    "ethrpc_batch_size",
    "ethrpc_concurrency",
    "wall_ms",
    "calls",
]
# The pass index was called `run` before the columns were renamed, so accept
# both rather than crashing on a CSV from an older build.
PASS_COLUMNS = ["pass", "run"]


def read(path):
    """Groups wall times by config, dropping each config's first pass.

    That first pass pays for the reqwest connection pool ramping up, which a
    6-second unbatched pass amortises away but a 250ms multicall pass does not —
    keeping it makes the fast configs look 2-5x worse than they are.
    """
    walls = defaultdict(list)
    calls = {}
    with open(path, newline="") as handle:
        reader = csv.DictReader(handle)
        columns = reader.fieldnames or []

        missing = [name for name in REQUIRED if name not in columns]
        if missing:
            sys.exit(f"{path}: missing column(s) {', '.join(missing)}")
        index = next((name for name in PASS_COLUMNS if name in columns), None)
        if index is None:
            sys.exit(f"{path}: need a {' or '.join(PASS_COLUMNS)} column")

        for row in reader:
            if int(row[index]) == 0:
                continue
            multicall = int(row["multicall_batch_size"])
            key = (
                multicall,
                int(row["ethrpc_batch_size"]),
                int(row["ethrpc_concurrency"]),
            )
            walls[key].append(int(row["wall_ms"]))
            calls[multicall] = int(row["calls"])
    if not walls:
        sys.exit(f"{path}: no usable rows (need more than one pass per config)")
    return walls, calls


def summarise(walls):
    """median, min and max per config."""
    return {
        key: (statistics.median(values), min(values), max(values))
        for key, values in walls.items()
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv", type=Path)
    parser.add_argument("out", type=Path, nargs="?", default=Path("multicall-bench.png"))
    parser.add_argument(
        "--subject",
        default="the working set",
        help="what was measured, for the subtitle",
    )
    parser.add_argument("--dpi", type=int, default=300)
    args = parser.parse_args()

    walls, calls = read(args.csv)
    stats = summarise(walls)

    sizes = sorted({mc for mc, _, _ in stats if mc != UNBATCHED})
    batches = sorted({b for _, b, _ in stats})
    concurrencies = sorted({c for _, _, c in stats})
    positions = {size: index for index, size in enumerate(sizes)}

    base = [stats[key][0] for key in stats if key[0] == UNBATCHED]
    if not base:
        sys.exit(f"{args.csv}: no unbatched rows to compare against")

    plt.rcParams.update(
        {
            "font.family": "sans-serif",
            "font.size": 11,
            "figure.facecolor": SURFACE,
            "axes.facecolor": SURFACE,
        }
    )
    figure, (top, bottom) = plt.subplots(
        2,
        1,
        figsize=(13, 10),
        height_ratios=[2.5, 1],
        gridspec_kw={"hspace": 0.42},
    )

    for axes in (top, bottom):
        axes.set_yscale("log")
        axes.set_xlim(-0.45, len(sizes) - 0.55)
        axes.grid(axis="y", color=GRIDLINE, linewidth=0.8, zorder=0)
        axes.set_axisbelow(True)
        for edge in ("top", "right"):
            axes.spines[edge].set_visible(False)
        for edge in ("left", "bottom"):
            axes.spines[edge].set_color(BASELINE)
        axes.tick_params(colors=MUTED, labelcolor=MUTED, length=4)

    # ─── panel A: wall time ─────────────────────────────────────────────────
    top.axhspan(
        min(base),
        max(base),
        facecolor=BAND,
        edgecolor=BASELINE,
        linewidth=0.8,
        zorder=1,
    )
    centre = (min(base) * max(base)) ** 0.5
    top.text(
        -0.36,
        centre * 1.5,
        "unbatched — one eth_call per read",
        color=INK_2,
        fontsize=12,
        va="center",
    )
    top.text(
        -0.36,
        centre / 1.5,
        f"{min(base) / 1000:.1f} s – {max(base) / 1000:.1f} s, "
        "depending only on the transport settings",
        color=MUTED,
        fontsize=10.5,
        va="center",
    )

    # Nudge each series off the shared category position so one config's
    # whiskers do not sit on another's.
    span = 0.055
    count = len(batches) * len(concurrencies)
    for index, (batch, concurrency) in enumerate(
        (b, c) for b in batches for c in concurrencies
    ):
        dash, marker = STYLES[concurrencies.index(concurrency)]
        colour = HUES[batches.index(batch) % len(HUES)]
        offset = (index - (count - 1) / 2) * span

        points = [(positions[s], stats[(s, batch, concurrency)]) for s in sizes
                  if (s, batch, concurrency) in stats]
        x = [position + offset for position, _ in points]
        median = [value[0] for _, value in points]
        low = [value[0] - value[1] for _, value in points]
        high = [value[2] - value[0] for _, value in points]

        top.errorbar(
            x,
            median,
            yerr=[low, high],
            color=colour,
            linestyle=dash,
            marker=marker,
            markersize=7,
            linewidth=1.8,
            elinewidth=1.4,
            capsize=4,
            zorder=3,
            label=f"ethrpc batch {batch} · concurrency {concurrency}",
        )

    top.set_ylabel("wall time per pass", color=INK_2)
    top.set_ylim(200, 7000)
    top.set_yticks([200, 500, 1000, 2000, 5000])
    top.set_yticklabels(["200 ms", "500 ms", "1 s", "2 s", "5 s"])
    top.set_xticks(range(len(sizes)))
    top.set_xticklabels([])
    top.minorticks_off()

    figure.suptitle(
        "Balance fetching: Multicall3 against reading every balance on its own",
        x=0.055,
        y=0.977,
        ha="left",
        fontsize=17,
        color=INK,
    )
    figure.text(
        0.055,
        0.936,
        f"{args.subject}  ·  lower is better",
        ha="left",
        fontsize=11.5,
        color=INK_2,
    )
    figure.text(
        0.055,
        0.911,
        "median of 9 timed passes, whiskers min–max  ·  first pass excluded, "
        "it pays for the connection pool ramping up",
        ha="left",
        fontsize=11.5,
        color=MUTED,
    )

    # Six entries anywhere inside the plot would land on the data or the band.
    legend = top.legend(
        loc="upper left",
        bbox_to_anchor=(-0.005, -0.06),
        ncol=3,
        frameon=False,
        fontsize=10.5,
        labelcolor=INK_2,
        handlelength=2.4,
        columnspacing=2.4,
    )
    legend.set_zorder(5)

    # ─── panel B: RPC volume ────────────────────────────────────────────────
    bottom.axhline(calls[UNBATCHED], color=BASELINE, linewidth=1.6, zorder=2)
    bottom.text(
        -0.36,
        calls[UNBATCHED] / 2.4,
        f"unbatched: {calls[UNBATCHED]}",
        color=INK_2,
        fontsize=10.5,
        va="center",
    )

    volume = [calls[size] for size in sizes]
    bottom.plot(
        range(len(sizes)),
        volume,
        color=HUES[0],
        marker="o",
        markersize=7,
        linewidth=1.8,
        zorder=3,
    )
    for position, value in enumerate(volume):
        bottom.annotate(
            f"{value}",
            (position, value),
            textcoords="offset points",
            xytext=(0, 11),
            ha="center",
            fontsize=10.5,
            color=INK_2,
        )

    bottom.set_title(
        "Logical JSON-RPC calls per pass — set by the multicall batch size alone, "
        "identical under every transport setting",
        loc="left",
        fontsize=11.5,
        color=INK,
        pad=14,
    )
    bottom.set_ylabel("calls", color=INK_2)
    bottom.set_ylim(10, 11000)
    bottom.set_yticks([10, 100, 1000, 10000])
    bottom.set_yticklabels(["10", "100", "1k", "10k"])
    bottom.set_xticks(range(len(sizes)))
    bottom.set_xticklabels([str(size) for size in sizes])
    bottom.set_xlabel("queries per Multicall3 call", color=INK_2)
    bottom.minorticks_off()

    figure.subplots_adjust(left=0.075, right=0.985, top=0.875, bottom=0.085)
    figure.savefig(args.out, dpi=args.dpi, facecolor=SURFACE)
    print(f"wrote {args.out} ({args.dpi} dpi)")


if __name__ == "__main__":
    main()

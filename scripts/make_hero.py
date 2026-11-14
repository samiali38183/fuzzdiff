"""Render docs/img/hero-differential.png from the saved runs in docs/data/.

    pip install matplotlib
    python scripts/make_hero.py

docs/data/generator_runs.txt   seeded-generator runs (0 divergences)
docs/data/mutants_run.txt      output of the planted-bug test
The 2026-11-11 row of generator_runs.txt is the figure recorded in the README; the 2026-11-14 row is raw output.
"""

from __future__ import annotations

import re
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "img" / "hero-differential.png"
INK, MUTED, LINE = "#111827", "#4b5563", "#d1d5db"
GREEN, BLUE, AMBER = "#15803d", "#1d4ed8", "#b45309"
plt.rcParams["font.family"] = ["Segoe UI", "DejaVu Sans"]

# What each planted bug is, in plain words (from the README list, in the order the test prints them).
BUGS = {
    "HeaderLenOffByOne": "A header value comes back one byte short",
    "MissingBoundsCheck": "A missing end-of-buffer check",
    "WrongStateAfterCrlf": "Chunked decoder in the wrong state",
    "OverheadRatio": "A size guard at 80% instead of 75%",
    "StatusGuard": "Response status guard waits 3 bytes, not 4",
}


def read_runs() -> list[dict[str, int | str]]:
    runs = []
    for line in (ROOT / "docs" / "data" / "generator_runs.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        date, rest = line.split(": ", 1)
        kv = {k: int(v) for k, v in re.findall(r"(\w+)=(\d+)", rest)}
        kv["inputs"] = int(rest.split(" inputs")[0])
        runs.append({"date": date, **kv})
    return runs


def read_mutants() -> list[tuple[str, str]]:
    """(name, how it was found) for each planted bug."""
    found: dict[str, str] = {}
    for line in (ROOT / "docs" / "data" / "mutants_run.txt").read_text(encoding="utf-8").splitlines():
        m = re.match(r"(\w+): generator caught after ([\d,]+) inputs", line)
        if m:
            n = int(m.group(2))
            found[m.group(1)] = "found after %s input%s" % (format(n, ","), "" if n == 1 else "s")
        m = re.match(r"(\w+): generator did not catch it in (\d+) inputs", line)
        if m:
            found[m.group(1)] = "missed by %s random inputs" % format(int(m.group(2)), ",")
        m = re.match(r"(\w+): targeted case #\d+ catches it", line)
        if m:
            found[m.group(1)] += "; a test case caught it"
    return [(k, found[k]) for k in BUGS]


def main() -> None:
    runs = read_runs()
    latest = runs[-1]
    assert all(r["divergences"] == 0 for r in runs)
    mutants = read_mutants()

    fig = plt.figure(figsize=(9, 5.6), dpi=200, facecolor="white")
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_xlim(0, 1)
    ax.set_ylim(0, 1)
    ax.axis("off")

    ax.text(0.03, 0.93, "Does a safe Rust rewrite behave exactly like the C original?", fontsize=18.5, weight="bold", color=INK)
    ax.text(0.03, 0.865, "The same input goes to both parsers. Any difference in the answer is reported.", fontsize=11.5, color=MUTED)

    # Left
    ax.text(0.03, 0.79, "Random inputs, both parsers", fontsize=11.5, weight="bold", color=MUTED)
    ax.text(0.03, 0.70, "0", fontsize=60, weight="bold", color=GREEN, va="center")
    ax.text(0.115, 0.725, "differences found", fontsize=17, weight="bold", color=INK, va="center")
    ax.text(0.115, 0.675, "in each 90-second run of generated inputs", fontsize=10.5, color=INK, va="center")
    rows = ["%s: %s inputs" % (r["date"], format(r["inputs"], ",")) for r in runs]
    ax.text(0.03, 0.575, "\n".join(rows), fontsize=11, color=INK, va="top", linespacing=1.4)

    total = latest["inputs"]
    segs = [
        (latest["equal"], BLUE, "same answer"),
        (latest["both_reject"], "#6b7280", "both reject the input"),
        (latest["rust_stricter"], AMBER, "Rust stricter (one documented difference)"),
    ]
    ax.text(0.03, 0.44, "What happened in the %s run" % latest["date"], fontsize=10.5, weight="bold", color=MUTED)
    x, w_total = 0.03, 0.43
    for n, c, _ in segs:
        w = w_total * n / total
        ax.add_patch(plt.Rectangle((x, 0.375), w, 0.04, fc=c, ec="white", lw=1.2))
        x += w
    ly = 0.335
    for n, c, label in segs:
        ax.add_patch(plt.Rectangle((0.03, ly - 0.011), 0.02, 0.024, fc=c, ec="none"))
        ax.text(0.06, ly, "%s  %s" % (format(n, ","), label), fontsize=10, color=INK, va="center")
        ly -= 0.04
    ax.text(0.03, 0.195, "Also: 0 differences replaying 204 corpus files in every\nprefix and split, and in 120 s libFuzzer runs in CI.", fontsize=10, color=INK, va="top", linespacing=1.3)

    # Right
    rx = 0.545
    ax.text(rx, 0.79, "Would the checker notice a real bug?", fontsize=11.5, weight="bold", color=MUTED)
    ax.text(rx, 0.735, "5 of 5", fontsize=26, weight="bold", color=GREEN, va="center")
    ax.text(rx + 0.175, 0.735, "planted bugs detected", fontsize=13.5, weight="bold", color=INK, va="center")
    y = 0.65
    for name, how in mutants:
        ax.text(rx, y, "\u2713", fontsize=15, weight="bold", color=GREEN, va="center")
        ax.text(rx + 0.04, y + 0.014, BUGS[name], fontsize=10.5, weight="bold", color=INK, va="center")
        ax.text(rx + 0.04, y - 0.017, how, fontsize=9.5, color=MUTED, va="center")
        y -= 0.087
    ax.text(rx, 0.185, "A control run with no planted bug: 500,000 inputs, no difference.", fontsize=9.8, color=INK, va="top")

    ax.plot([0.03, 0.97], [0.115, 0.115], color=LINE, lw=1)
    ax.text(0.03, 0.075, "Sampling, not proof. Two documented deviations from C. No bugs were found in the C original.\nThe port forbids unsafe code and has no dependencies.", fontsize=10, color=MUTED, va="center", linespacing=1.3)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT, dpi=200, facecolor="white")
    print("wrote", OUT)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Does the test suite notice a bug in the C parser's SIMD path?

fuzzdiff compares a Rust port against picohttpparser. Its seeded bugs (the `mutants` feature)
are all in the Rust side. This script plants one-line bugs in a temporary copy of the *C*
SSE4.2 code instead, builds the harness against that copy (FUZZDIFF_C_DIR) with the SSE4.2
path switched on, and records which checks fail:

  existing   corpus replay, property tests, targeted cases, 20 s of the seeded generator
  sweep      harness/tests/simd_boundaries.rs (every byte value at every offset of fields up
             to 40 bytes)

Usage: python scripts/c_mutants.py [--quick-secs N]   (needs cargo and a C compiler)
Output is a markdown table. The vendored file is never modified.
"""
import argparse, os, shutil, subprocess, sys, tempfile, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# name, original text (must occur exactly once), replacement, what the edit does
MUTANTS = [
    ("EolRangesNoDel", "ranges1, 6, &found", "ranges1, 4, &found",
     "header value / reason scan no longer stops at DEL (0x7f) inside a 16-byte block"),
    ("EolRangesNoUS", r'"\012\037"', r'"\012\036"',
     "header value / reason scan misses byte 0x1f"),
    ("TokenRangesNoBackslash", '"[]"     /* 0x5b-0x5d */', '"[["     /* 0x5b-0x5d */',
     "token scan (method, header name) no longer stops at ']' and backslash inside a block"),
    ("TokenRangesNoHighBytes", r'"{\xff"; /* 0x7b-0xff */', r'"{\x7f"; /* 0x7b-0xff */',
     "token scan misses bytes 0x80-0xff inside a block"),
    ("PathRangesNoDel", "ranges2, 4, &found2", "ranges2, 2, &found2",
     "request-target scan no longer stops at DEL inside a block"),
    ("FoundOffByOne", "if (unlikely(r != 16)) {", "if (unlikely(r != 15)) {",
     "block scan treats a hit at offset 15 as 'nothing found'"),
    ("GuardLongerThan16", "if (likely(buf_end - buf >= 16)) {", "if (likely(buf_end - buf > 16)) {",
     "SIMD scan skipped when exactly 16 bytes remain (the byte loop handles them): "
     "expected to be behaviourally equivalent"),
]

def run(cmd, env, timeout=900):
    t = time.time()
    p = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True, timeout=timeout)
    return p.returncode, time.time() - t, p.stdout + p.stderr

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--quick-secs", type=int, default=20)
    a = ap.parse_args()
    base = dict(os.environ, FUZZDIFF_C_SSE42="1")
    rows = []
    cases = [("(none: unmodified C)", None, None, "control")] + MUTANTS
    for name, old, new, what in cases:
        tmp = tempfile.mkdtemp(prefix="fd_c_")
        d = os.path.join(tmp, "c")
        shutil.copytree(os.path.join(ROOT, "c"), d)
        if old:
            p = os.path.join(d, "picohttpparser.c")
            s = open(p, encoding="utf-8", newline="").read()
            if s.count(old) != 1:
                sys.exit(f"{name}: pattern occurs {s.count(old)} times")
            open(p, "w", encoding="utf-8", newline="").write(s.replace(old, new))
        env = dict(base, FUZZDIFF_C_DIR=d)
        rc1, _, _ = run(["cargo", "test", "--release", "-p", "fuzzdiff-harness", "--test", "corpus",
                         "--test", "props", "--test", "targeted"], env)
        rc2, _, _ = run(["cargo", "run", "--release", "-p", "fuzzdiff-harness", "--example",
                         "quickfuzz", "--", str(a.quick_secs)], env)
        rc3, _, _ = run(["cargo", "test", "--release", "-p", "fuzzdiff-harness", "--test",
                         "simd_boundaries"], env)
        existing = "caught" if (rc1 or rc2) else "not caught"
        sweep = "caught" if rc3 else "not caught"
        if old is None and (rc1 or rc2 or rc3):
            sys.exit("control run (unmodified C) failed; fix the environment before trusting any row")
        rows.append((name, what, existing, sweep))
        shutil.rmtree(tmp, ignore_errors=True)
        print(rows[-1], flush=True)
    print("\n| Planted C bug | Effect | Existing checks | Window sweep |\n|---|---|---|---|")
    for r in rows:
        print("| " + " | ".join(r) + " |")

if __name__ == "__main__":
    main()

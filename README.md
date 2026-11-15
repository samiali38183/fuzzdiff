# fuzzdiff

[![ci](https://github.com/samiali38183/fuzzdiff/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/samiali38183/fuzzdiff/actions/workflows/ci.yml)
[![fuzz](https://github.com/samiali38183/fuzzdiff/actions/workflows/fuzz.yml/badge.svg)](https://github.com/samiali38183/fuzzdiff/actions/workflows/fuzz.yml)

HTTP servers read bytes from strangers, and the parser that does it is a common place for
memory-safety bugs. fuzzdiff is a Rust port of [picohttpparser](https://github.com/h2o/picohttpparser),
a small C parser for HTTP/1.x requests, responses, headers and chunked bodies, plus a harness that
feeds the same input to the C original and the port and reports any difference in verdict, consumed
length or parsed fields. I wanted to find out whether a safe rewrite behaves like the original by
testing it, not by saying so.

The port forbids `unsafe` and has no dependencies. The C side is vendored unmodified and runs under
AddressSanitizer and UBSan in CI, in both its portable and its SSE4.2 code paths. The harness is
itself tested: bugs I planted in the port have to be detected, and so do bugs planted in the C
SIMD code. A third parser, `httparse`, is compared too.

## What was checked

* **Differential testing.** A replayed corpus (picohttpparser's own tests plus hand-written edge
  cases, every prefix and every two-way chunked split), property tests, a seeded generator, a
  deterministic sweep that puts every byte value at every offset of the C parser's 16-byte SIMD
  windows, and libFuzzer on a differential target. No divergence has been observed.
* **Memory safety.** C under ASan and UBSan on the portable and SSE4.2 paths; Miri on the Rust crate.
  No reports. Miri cannot run the C side, so it covers the port only.
* **Does the harness notice bugs?** Five seeded bugs in the port, all detected, one only by a
  hand-written case because random inputs never reach it. Six one-line bugs in the C SIMD code,
  all caught by the existing checks (and by the sweep); one edit that cannot change behaviour was
  correctly not caught.
* **Disagreement with another parser.** `httparse` and picohttpparser differ in a fixed set of
  classes (obsolete line folding, extra spaces in the request line, and others), each pinned by a
  test so a new class fails CI.

The seeded bugs were planted by me, so they show the harness can see a bug, not that it would see
every one. No bug was found in the C original, and a clean differential run does not prove the two
parsers equivalent: it means the inputs tried did not separate them. The port deviates from C in
two documented places ([DEVIATIONS](docs/DEVIATIONS.md)) and is about 1.4x to 1.5x slower on head
parsing.

## Quick start

```
git clone https://github.com/samiali38183/fuzzdiff && cd fuzzdiff
cargo test --workspace
cargo run --release -p fuzzdiff-harness --example quickfuzz -- 60
cargo test --release -p fuzzdiff-harness --features mutants --test mutants -- --nocapture
```

The first `cargo test` builds the vendored C with the `cc` crate and runs the unit tests, the corpus
replay and the property tests (`harness/tests/`). The `quickfuzz` example runs the seeded generator
against both parsers for the given number of seconds and exits non-zero on a divergence. The last
line switches each planted bug on in turn and checks the harness reports it. The two `cargo test`
lines are the ones `.github/workflows/ci.yml` runs on Ubuntu and Windows; `quickfuzz` is the local
stand-in for the libFuzzer job in `fuzz.yml`. The `Makefile` has the same three as `make test`,
`make quickfuzz` and `make mutants`.

Rust stable and a C compiler are enough. `make` lists the other targets (Miri, coverage, benchmarks,
libFuzzer on Linux).

```rust
use fuzzdiff::{parse_request, Header, Status};

let buf = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
let mut headers = [Header::EMPTY; 16];
match parse_request(buf, &mut headers, 0)? {
    Status::Complete(req) => println!("{:?} {:?}", req.method, req.path),
    Status::Partial => { /* read more bytes, call again with last_len = buf.len() */ }
}
```

## Design

* The port is one dependency-free crate: `src/message.rs` (request, response and header-block
  parsing, `Config`), `src/chunked.rs` (`ChunkedDecoder`), `src/error.rs`. It is
  `#![forbid(unsafe_code)]` (`src/lib.rs`, enforced again by `[lints.rust]` in `Cargo.toml`), and
  indexes with `get` and checked slicing rather than `[]`.
* It is not a line-by-line translation. C pointer scans and `CHECK_EOF` macros became slice
  scans and an internal `Stop { Partial, Fail }` type with `?`; the chunked decoder's fall-through
  `switch` became an enum matched in a loop. Odd-looking C behaviours (which of partial and failed is
  decided first, how many bytes `HTTP/1.` needs before it is judged) were kept so the harness can
  compare them. Details in [docs/DESIGN.md](docs/DESIGN.md).
* The C original is vendored byte-identical in `c/` (hashes checked by `scripts/verify_vendor.py`
  in CI) and built by `harness/build.rs` with the `cc` crate. The shim `c/shim.c` copies each input
  into a heap block of exactly its size, so ASan reports an over-read instead of it landing in spare
  capacity.
* `harness/src/ffi.rs` is the only file with `unsafe`: the `extern "C"` declarations and the calls
  into the shim, each with a SAFETY comment.
* `compare_case` in `harness/src/lib.rs` runs both parsers on one `Case` and returns `Equal`,
  `BothReject`, `RustStricter(D1|D2)` for the two documented deviations, or `Divergence` (results
  differ, Rust panicked, or C returned an offset outside the input). For chunked bodies the whole
  session is compared: every call's return value, decoded length, `is_in_data` flag, decoded bytes
  and leftover bytes.
* `Case::from_bytes` turns raw fuzzer bytes into a case: 4 control bytes choose the entry point,
  `max_headers`, `last_len` and the chunk split size, the rest is the payload. One libFuzzer target
  (`fuzz/fuzz_targets/diff.rs`) therefore covers all four entry points and the incremental path.
* Property tests (`harness/tests/props.rs`, proptest, 3000 cases by default) draw inputs from the
  generator and mutations in `harness/src/generate.rs` and assert agreement and no Rust panic.
  `harness/tests/corpus.rs` replays `corpus/` with every prefix and chunked split;
  `harness/tests/simd_boundaries.rs` sweeps every byte value at every offset of the C 16-byte windows.
* Planted bugs in the port live in `src/mutants.rs` behind the `mutants` cargo feature and are
  switched on per thread; without the feature the hooks are `const fn` returning `false`, so the
  shipped parser contains none of that code. `harness/tests/mutants.rs` enables each one and runs the
  generator until a divergence is reported. `scripts/c_mutants.py` does the same from the C side:
  it edits a temporary copy of the C file (never the vendored one) and reruns the checks.
* `httparse` is a third oracle (`harness/src/httparse_oracle.rs`); the known disagreement classes
  are pinned in `harness/tests/httparse_classes.rs` and the table in
  [docs/DIFFERENTIAL_HTTPARSE.md](docs/DIFFERENTIAL_HTTPARSE.md) is checked against the code in CI.

## Limitations

* Fuzz time is small. `fuzz.yml` runs each libFuzzer leg for 120 s on pull requests; the weekly
  3600 s schedule had not produced a run as of 2026-11-15 ([docs/EVIDENCE.md](docs/EVIDENCE.md)).
  Local runs use the deterministic generator, which has no coverage feedback.
* libFuzzer, ASan and UBSan run only on Linux in CI. Local Windows builds (MSVC) have no sanitizer,
  and Miri covers the pure-Rust crate only because it cannot execute the C side or the FFI harness.
* Two cases are not reachable by the fuzz targets: the chunked framing-overhead guard (needs over
  100 KiB of framing, above `-max_len=4096`) and `last_len > len` (D2, clamped in the fuzz entry point).
  Both are covered by targeted or unit tests instead.
* Only 64-bit x86 has been tested (Ubuntu and Windows). No 32-bit, ARM or big-endian target has run.
* Scope is HTTP/1.x text parsing as picohttpparser does it: no URI parsing, no header semantics, no
  HTTP/2. Behaviour that cannot be compared by running both sides (decoder reuse after a final
  result, `int` versus `usize` return width, NULL arguments, out-parameters on failure) is listed in
  [docs/DEVIATIONS.md](docs/DEVIATIONS.md) rather than tested.
* The five planted Rust bugs and six planted C bugs were chosen by the author; they show the harness
  can see those classes, not that it would see every bug.

## Where the detail is

[docs/EVIDENCE.md](docs/EVIDENCE.md) has every result with its run category and source: test
counts, the CI fuzz legs, the seeded-bug output, the C mutation table and what has not run. Also in
`docs/`: [DESIGN](docs/DESIGN.md), [DEVIATIONS](docs/DEVIATIONS.md), [CVE_CLASSES](docs/CVE_CLASSES.md),
[DIFFERENTIAL_HTTPARSE](docs/DIFFERENTIAL_HTTPARSE.md), [BENCHMARKS](docs/BENCHMARKS.md),
[FINDINGS](docs/FINDINGS.md).

MIT for the Rust code; the vendored C is MIT or Perl, see [c/LICENSE](c/LICENSE).

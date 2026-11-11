# Changelog

## 0.2.0

* Three-way differential: `httparse` 1.10.1 (pinned) is a third oracle in the harness. Curated
  cases, a deterministic 2,000,000-input sweep, generated tables and RFC 9112 / 9110 / 3986
  references in `docs/DIFFERENTIAL_HTTPARSE.md`; a test pins the disagreement classes so a new
  one fails CI.
* Two new named mutants behind the `mutants` feature (`OverheadRatio`, `StatusGuard`) for gaps
  a mutation sweep found in the 0.1.0 suite, with targeted cases that catch them.
* Miri CI job for the pure-Rust crate; a pseudo-random invariant test that also runs under Miri.
* Fuzzing: `diff` and `c_only` also run with the C side built with `-msse4.2`; the fuzz corpus
  is cached between runs; the weekly run is 3600 s per leg.
* README and docs: what "matches picohttpparser" means, the httparse comparison, updated
  measurements.
* No change to the parser's behaviour or public API.

## 0.1.0

Initial version.

* Rust port of picohttpparser: `parse_request`, `parse_response`, `parse_headers` and the
  incremental `ChunkedDecoder`, `#![forbid(unsafe_code)]`.
* Opt-in `Config::reject_obs_fold` (deviation D1) and `Error::InvalidLastLen` (D2).
* Differential harness against the vendored C library (commit dd573d8).
* Seed corpus (upstream test inputs and hand-written cases), property tests, three seeded
  bugs behind the `mutants` feature.
* cargo-fuzz targets `diff`, `rust_only`, `c_only`; CI and fuzz workflows (first runs green, see README).
* Criterion benchmarks against the C parser.

# Changelog

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

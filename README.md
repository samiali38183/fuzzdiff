# fuzzdiff

[![ci](https://github.com/samiali38183/fuzzdiff/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/samiali38183/fuzzdiff/actions/workflows/ci.yml)
[![fuzz](https://github.com/samiali38183/fuzzdiff/actions/workflows/fuzz.yml/badge.svg)](https://github.com/samiali38183/fuzzdiff/actions/workflows/fuzz.yml)

A Rust reimplementation of [picohttpparser](https://github.com/h2o/picohttpparser) (the HTTP/1.x
request, response, header and chunked-body parser written in C), with a harness that runs the
C original and the Rust port side by side on the same input and reports any difference.

Status: the code, tests and benchmarks ran locally on Windows; the test matrix, the sanitizer
builds and the three libFuzzer targets have run on GitHub Actions and passed. See
"What has and has not run".

## Why this exists

Network parsers written in C keep having the same bugs: reads past the end of a buffer, length
arithmetic that wraps, off-by-ones, half-initialised output. Rewriting one in a memory-safe
language is a fair answer, but only if the rewrite behaves like the original. This project is
a small, complete example of doing that and checking it, instead of asserting it.

picohttpparser was chosen because it is small (715 lines of C), permissively licensed
(MIT or Perl), hostile-input facing, and full of the pointer idioms that cause these bugs.
The reasoning and the alternatives are in [docs/DESIGN.md](docs/DESIGN.md).

## What is in the repository

* `src/`: the Rust crate. `#![forbid(unsafe_code)]`, no dependencies, no panics by
  construction, about 990 lines including docs and the seeded test bugs. Request, response,
  header block and an incremental in-place chunked decoder.
* `c/`: picohttpparser at commit dd573d8 (unmodified, hashes checked) and a thin shim that
  returns offsets instead of pointers and copies inputs into exact-size heap blocks.
* `harness/`: builds the C with the `cc` crate, `compare()` and the `Outcome` classification,
  tests, generators, benchmarks.
* `fuzz/`: cargo-fuzz targets `diff`, `rust_only` and `c_only`.
* `corpus/`: 204 inputs: 94 taken from picohttpparser's own test suite (MIT) and 110
  hand-written edge cases, plus 6 benchmark inputs.
* `docs/`: [DESIGN](docs/DESIGN.md), [DEVIATIONS](docs/DEVIATIONS.md),
  [CVE_CLASSES](docs/CVE_CLASSES.md), [BENCHMARKS](docs/BENCHMARKS.md),
  [FINDINGS](docs/FINDINGS.md).

## Using the crate

```rust
use fuzzdiff::{parse_request, Header, Status};

let buf = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
let mut headers = [Header::EMPTY; 16];
match parse_request(buf, &mut headers, 0)? {
    Status::Complete(req) => println!("{:?} {:?}", req.method, req.path),
    Status::Partial => { /* read more bytes, call again with last_len = buf.len() */ }
}
```

The behaviour matches the C library except for two documented differences, see
[docs/DEVIATIONS.md](docs/DEVIATIONS.md).

## How the comparison works

`compare(bytes)` decodes fuzzer bytes into a case (which of the four entry points, header
capacity, `last_len`, how a chunked body is split across calls), runs C and Rust, and returns

* `Equal`: same consumed length, fields and bytes, or both partial
* `BothReject`: both returned an error
* `RustStricter(D1 | D2)`: one of the two documented differences
* `Divergence`: a bug in one of the two; fails the tests and the CI fuzz job

## Demo: the harness catching seeded bugs

The `mutants` feature adds three deliberate bugs to the Rust parser (header value one byte
short past 16 bytes; a missing end-of-buffer guard after the HTTP version; the wrong state
after a trailer line in the chunked decoder). This is the output of a real run on 2026-11-10,
Windows, release build:

```
$ cargo test --release -p fuzzdiff-harness --features mutants --test mutants -- --nocapture

mutant detection (deterministic generator, seed = input index + 1)
HeaderLenOffByOne: caught after 3 inputs in 49.1µs as ResultMismatch
MissingBoundsCheck: caught after 286 inputs in 1.2687ms as RustPanic
  first failing input: Request last_len=0 max_headers=2 strict=false trailer=false pieces=[29] data="\r\nGET /q=0.9&;;q=0.9 HTTP/1.1"
WrongStateAfterCrlf: caught after 8 inputs in 35.5µs as ResultMismatch
control (no mutant): 500000 inputs, no divergence, 1.5522451s
test harness_detects_every_seeded_bug ... ok
```

(Lines showing the first failing input for the other two mutants are omitted here for length; the test prints them.)

The input that trips `MissingBoundsCheck` is a request cut off right after `HTTP/1.1`.
C answers "partial". The mutant indexes one byte past the end; in C that would be an
over-read, here it is a panic and the harness reports it. These mutants are easy for the
generator (microseconds to milliseconds). That shows the plumbing works and that each bug
class is visible, not that the harness would find every subtle bug.

## Running things

```
make test         # cargo test --workspace
make lint         # fmt, clippy -D warnings (with and without the mutants feature)
make mutants      # the demo above
make cover        # cargo llvm-cov, fails under 90% lines
make bench        # criterion, C vs Rust
make quickfuzz    # 60 s of the seeded generator against both implementations
make seeds        # write the libFuzzer seed corpus (Linux fuzzing below)
make fuzz-diff fuzz-rust fuzz-c   # libFuzzer targets, need Linux, nightly, clang, cargo-fuzz
make asan         # cargo test with the C side under ASan and UBSan (Linux)
```

Needs a C compiler (MSVC Build Tools or gcc/clang) for the `cc` crate. Build switches for CI
are documented at the top of `harness/build.rs`.

## Results

All numbers are from real runs on one Windows machine (Ryzen 5 5600G, rustc 1.99.1, MSVC 19.39).

| Check | Result |
|---|---|
| `cargo test --workspace` | 29 tests pass (10 crate known-answer, 12 corpus replay, 5 property tests, 2 doc tests), plus 1 mutant test with `--features mutants` |
| Property tests | 5 properties, 3000 cases each by default |
| Corpus replay | every file through every entry point, every prefix length with incremental `last_len`, every two-way chunked split, every byte value in every position of a valid message; 0 divergences |
| Seeded generator, 90 s release run | 27,639,000 inputs, 0 divergences (14.3M equal, 12.9M both reject, 0.46M rust-stricter by D1) |
| Seeded bugs | 3 of 3 detected (3, 286 and 8 inputs); 500,000-input control without a bug: clean |
| Line coverage of `src/` | 95.88% (cargo-llvm-cov, tests from both crates) |
| `cargo clippy -D warnings`, `cargo fmt --check` | clean, also with `mutants` |
| actionlint on both workflows | clean |
| Speed against C | Rust is about 1.4x to 1.5x slower on head parsing, level on chunked decoding, see [docs/BENCHMARKS.md](docs/BENCHMARKS.md) |
| Bugs found in the C original | none |

## What has and has not run

Ran locally: everything in the table above.

Ran in CI, run [42961733968](https://github.com/samiali38183/fuzzdiff/actions/runs/42961733968) (`ci`) and run [42961733885](https://github.com/samiali38183/fuzzdiff/actions/runs/42961733885) (`fuzz`), both on the pull
request that fixed the first run, all jobs green:

* `ci`: tests on Ubuntu and Windows, clippy, `mutants` detection, cargo-deny, and coverage
  (`--fail-under-lines 90`, passed).
* `ci`, sanitized C: the C parser is compiled with `-fsanitize=address,undefined
  -fno-sanitize-recover=undefined` by clang, and `cargo test --workspace --release` runs with
  `halt_on_error=1`. It passed in two variants, the portable C path and `-msse4.2` (the SIMD
  path of the C parser, which the Windows runs could not reach). ASan and UBSan reported
  nothing in the C code on the tests' inputs.
* `fuzz`, libFuzzer on nightly, 120 s per target, seeded from `corpus/`, as printed by libFuzzer:

| Target | What it runs | Executions | Time | Average exec/s | Corpus at end | Result |
|---|---|---|---|---|---|---|
| `diff` | C (ASan, coverage) against Rust, panics on a divergence | 5,027,708 | 121 s | 41,551 | 636 units | no crash, no divergence |
| `rust_only` | Rust only, debug assertions, output invariants | 7,762,840 | 121 s | 64,155 | 509 units | no crash |
| `c_only` | C only, ASan and UBSan | 6,509,270 | 121 s | 53,795 | 434 units | no crash, no sanitizer report |

Two minutes per target is a smoke run, not an assurance. The `diff` and `c_only` fuzz targets
build the C side without `-msse4.2`, so libFuzzer exercised the portable C path; the SIMD
path was covered only by the `cargo test` sanitizer job above.

Not run: Miri.

The seeded-generator run above is a stand-in for fuzzing, not a replacement. It is blind
(no coverage feedback), so libFuzzer should reach states it does not.

## License

The Rust code is MIT, see [LICENSE](LICENSE). The vendored C code is picohttpparser, MIT or
Perl dual license, copyright Kazuho Oku, Tokuhiro Matsuno, Daisuke Murase and Shigeo
Mitsunari; its text is in [c/LICENSE](c/LICENSE) and `corpus/` includes inputs from its test
suite.

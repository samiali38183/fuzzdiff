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

```
git clone https://github.com/samiali38183/fuzzdiff && cd fuzzdiff
cargo test --workspace
cargo test --release -p fuzzdiff-harness --features mutants --test mutants -- --nocapture
```

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

## Where the detail is

[docs/EVIDENCE.md](docs/EVIDENCE.md) has every result with its run category and source: test
counts, the CI fuzz legs, the seeded-bug output, the C mutation table and what has not run. Also in
`docs/`: [DESIGN](docs/DESIGN.md), [DEVIATIONS](docs/DEVIATIONS.md), [CVE_CLASSES](docs/CVE_CLASSES.md),
[DIFFERENTIAL_HTTPARSE](docs/DIFFERENTIAL_HTTPARSE.md), [BENCHMARKS](docs/BENCHMARKS.md),
[FINDINGS](docs/FINDINGS.md).

MIT for the Rust code; the vendored C is MIT or Perl, see [c/LICENSE](c/LICENSE).

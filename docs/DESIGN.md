# Design

## Why picohttpparser

The aim was a parser that is small enough to port faithfully in a few days, hostile-input
facing, written in C with the usual pointer arithmetic, and permissively licensed so the
original can be vendored.

Candidates looked at:

* picohttpparser (HTTP/1.x request line, response line, headers, chunked decoder). 715 lines of
  C in one file plus a 96 line header. MIT or Perl dual license. Four entry points with a flat
  C API that is easy to wrap. Has its own test suite (545 lines) to seed a corpus from.
* A DER/TLV parser. Good for length-overflow bugs, but there is no single small
  permissively licensed reference with a precise spec subset to compare against, and DER
  validity rules are a larger design problem than a port.
* A CBOR or MessagePack decoder. Small, but the interesting risk (allocation from an
  attacker-controlled length) is about the allocator policy more than the parser.
* A tar header parser. Fixed-size fields make it too easy; little state.

picohttpparser wins on size, API shape and the mix of risk: it does pointer-based scanning
over a caller buffer, computes lengths by subtracting pointers, has a hand-written state
machine with fall-through in the chunked decoder, accumulates a size in a loop, and
modifies the buffer in place. Those are the places memory-safety bugs live in network
parsers (see `CVE_CLASSES.md`).

Upstream: https://github.com/h2o/picohttpparser, vendored at commit
dd573d8fa9264c646a400e96ce9f07dea6f156e4. The license text is in `c/LICENSE`. The upstream
repository has no LICENSE file, only the grant in the file headers and README, so that
header text is what was copied. The vendored files are byte-identical to upstream and their
hashes are checked by `scripts/verify_vendor.py`.

## Layout

```
src/              fuzzdiff crate: the Rust parser (forbid(unsafe_code))
  message.rs      request, response and header-block parsing, Config
  chunked.rs      ChunkedDecoder
  error.rs        Error
  mutants.rs      seeded bugs, compiled only with the `mutants` feature
c/                vendored C, plus shim.c/shim.h written for this project
harness/          the only crate with unsafe code
  build.rs        builds C with the cc crate, optional sanitizers
  src/ffi.rs      extern "C" declarations and safe wrappers
  src/lib.rs      Case, compare(), Outcome
  src/generate.rs deterministic generators and mutations
  tests/          corpus replay, proptest, mutant detection
fuzz/             cargo-fuzz targets (separate workspace, Linux and nightly)
corpus/           request/ response/ headers/ chunked/ and bench/
```

The Rust crate has no dependencies. All `unsafe` is in `harness/src/ffi.rs` (the FFI
declarations and calls) and each use has a SAFETY comment. The shim copies inputs into heap
blocks of exactly the right size, so AddressSanitizer sees an over-read of the C parser as
a heap overflow instead of silently reading spare capacity.

## Rust API shape

* Head parsers take `&[u8]`, a caller-supplied `&mut [Header]` and `last_len`, and return
  `Result<Status<T>, Error>`. `Status::Partial` is "need more bytes", not an error.
* Results borrow from the input (`&'a [u8]`), nothing is copied or allocated.
* `Config` carries the one opt-in behaviour change (D1). The free functions use the default.
* `ChunkedDecoder` is a struct with a `decode(&mut self, &mut [u8]) -> Decoded` method; the
  C struct fields that are meant to be private are private.

## How the port was written

Not a line-by-line translation. The C code scans with pointers and macros that `return NULL`
through an out parameter; the Rust code scans slices with `position` and `get`, uses an
internal `Stop { Partial, Fail(Error) }` with `?` instead of `CHECK_EOF` macros, and has the
chunked state machine as an enum matched in a loop instead of fall-through `switch` cases.
The behavioural details that look like accidents (the order in which `-1` and `-2` are
decided, that `HTTP/1.` needs nine bytes before it is judged, that the empty-method check
happens after the target is scanned) were kept on purpose, because the differential harness
compares them.

Indexing is done with `get` and checked slicing helpers rather than `[]`, so there is no
place the parser can panic by construction. The fuzz target `rust_only` checks that this
holds. The one direct index is inside the `MissingBoundsCheck` mutant.

## The harness

`compare_case(&Case)` runs both implementations on one input and returns:

* `Equal`: same consumed length and fields (or both partial, or identical chunked output).
* `BothReject`: both returned an error.
* `RustStricter(Deviation)`: a documented difference, see `DEVIATIONS.md`.
* `Divergence`: a bug. Kinds: the Rust code panicked, the results differ, or C returned an
  offset outside the input.

`Case::from_bytes` maps raw fuzzer bytes to a case (4 control bytes choose the entry point,
`max_headers`, `last_len` and the chunk split size, the rest is the payload), so one fuzz
target covers all four entry points and the incremental `last_len` path.

Chunked decoding is compared across a whole session: the shim feeds the same pieces to the
C decoder that the Rust side feeds to its decoder, and every call's return value, decoded
length and `is_in_data` flag is compared, plus all decoded bytes and the leftover bytes.

## Mutants

With the `mutants` feature, three deliberate bugs can be switched on per thread:

* `HeaderLenOffByOne`: header values longer than 16 bytes are one byte short.
* `MissingBoundsCheck`: the guard before the byte after `HTTP/1.x` is relaxed and the byte
  is read with a direct index. In C this would be a one byte over-read. In Rust it is a
  panic, which the harness reports as `RustPanic`.
* `WrongStateAfterCrlf`: after a trailer line the chunked decoder stays in the "middle of a
  line" state, so the blank line that ends the trailers is not recognised.

`harness/tests/mutants.rs` turns each on, runs the generator until the harness reports a
divergence, and prints how many inputs that took. It also runs the generator with no mutant
as a control. Without the feature the mutant hooks are `const fn` returning `false`.

## What is not covered

* Only HTTP/1.x text parsing. No URI parsing, no header semantics.
* Only the portable C path was compared locally (see DEVIATIONS.md).
* Fuzzing with libFuzzer has not run yet; it needs Linux, and runs in CI (`fuzz.yml`). What
  ran locally is the deterministic generator in `generate.rs`, documented in the README.

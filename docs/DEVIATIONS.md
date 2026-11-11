# Deviations from the C original

The goal is that the Rust port returns exactly what picohttpparser returns: the same
consumed length, the same partial/failed decision, the same fields, the same decoded
bytes. Two differences are intentional and are classified by the harness as
`RustStricter`. Anything else the harness sees is a `Divergence`, which is a bug and
fails CI. Identifiers below (D1, D2) are the ones used in `harness/src/lib.rs`, and a
test checks that every identifier has a section in this file.

Compared against picohttpparser commit dd573d8 (see `c/VENDORED.md`).

## D1

Opt-in rejection of folded header lines.

C: a header line that starts with a space or tab (after the first header) is accepted and
returned as a header with a NULL name and the line as its value.

Rust, default: identical to C.

Rust, `Config { reject_obs_fold: true }`: returns `Error::ObsFold`.

Why: line folding (obs-fold) is deprecated in RFC 9112 section 5.2, which says a server
must either reject the message or replace each fold with spaces before interpreting the
value. A parser that hands back a nameless header makes it easy for a caller to forget the
rule, and differing handling of folds between a proxy and a backend is a request
smuggling ingredient. It is off by default so the default behaviour stays byte-identical.

Harness class: the case runs with `strict = true`. If Rust returns `ObsFold` and C
returned a result (complete, partial or consumed bytes), the outcome is
`RustStricter(D1)`. If C also rejected the input, it is `BothReject`. In every other
situation the two must be equal.

## D2

`last_len` larger than the buffer.

C: `is_complete` starts scanning at `buf + last_len - 3` and compares against `buf_end`
with `==`. If `last_len` is more than `len + 3`, the start pointer is already past the end,
the `==` check never fires, and the loop reads beyond the buffer. This is a violation of
the documented contract (`last_len` is the length at the previous call), so it is a caller
bug, but the library gives no protection. This was found by reading the code; it has not
been demonstrated under AddressSanitizer in this repository.

Rust: `Error::InvalidLastLen`, checked before anything else is read.

Harness class: when a case has `last_len > len` the C function is not called (the behaviour
is undefined). The Rust result must be `InvalidLastLen`, and the outcome is
`RustStricter(D2)`. The fuzz entry point clamps `last_len` to `len`, so fuzzing never
reaches this class; it is covered by a unit test and a harness test.

## Differences with no harness class

These are real differences but cannot be compared by running both sides, so they are listed
rather than classified.

Decoder reuse. After `phr_decode_chunked` returns a result other than "incomplete", the C
decoder is left in whatever internal state it reached, and calling it again either continues
parsing nonsense or hits `assert(!"decoder is corrupt")`. The Rust `ChunkedDecoder` is
finished after `Complete` or `Error`, and later calls return
`Error(DecoderFinished)` without touching the buffer. The harness stops feeding after the
first final result.

Result type width. The C functions return `int`, so a head longer than `INT_MAX` bytes would
return a truncated value. Rust returns `usize`. This needs a multi-gigabyte input and is
not exercised.

NULL and invalid arguments. The C API accepts raw pointers, and passing NULL crashes (there
is an open upstream issue about it). The Rust API takes slices, so the situation cannot be
expressed.

Out parameters on failure. C writes partial results into its output parameters (for example
`num_headers`, `method`) even when it returns an error or "partial". Rust returns nothing
in those cases. The harness only compares fields when the parse is complete.

Error detail. C has one failure code (-1). Rust has a variant per reason. Every Rust `Err`
corresponds to a C -1 (or to D1 and D2).

Platform width. The maximum number of hex digits in a chunk size is `sizeof(size_t) * 2` in C
and `usize::BITS / 4` in Rust, which agree on every platform. Only 64-bit Windows has been
tested here.

The C SIMD path. picohttpparser has an SSE4.2 code path enabled by `__SSE4_2__`. MSVC does
not define that macro, so locally the portable path was compared. The CI job
`sanitized-c` builds the C side with `-msse4.2` as a second variant; that job has not run
yet.

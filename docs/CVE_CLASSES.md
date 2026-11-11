# C bug classes and how the Rust port prevents them

This maps four memory-safety bug classes that show up in C HTTP parsers to what the Rust
version does about each. The advisories are examples of the classes, mostly from other
parsers. picohttpparser itself has no published security advisory (its GitHub security
advisories page was empty when read), so nothing here claims a vulnerability in it.

Each advisory below was fetched and read on 2026-11-10. Where a page did not state a root
cause, the table says so instead of guessing.

## Advisories used as examples

| Class | Identifier | What the source says went wrong | Fetched |
|---|---|---|---|
| Integer overflow / signedness in chunk size | CVE-2013-2028 (nginx) | `ngx_http_parse_chunked` mishandles a large chunk size: an integer signedness error leading to a stack buffer overflow. The nginx patch adds a check for negative size or length. | CVE record https://cveawg.mitre.org/api/cve/CVE-2013-2028 (published 2013-07-18); advisory https://mailman.nginx.org/pipermail/nginx-announce/2013/000112.html (2013-05-07); patch https://nginx.org/download/patch.2013.chunked.txt |
| Buffer over-read in header parsing | CVE-2019-13503 (Mongoose 6.15) | Heap-based buffer over-read in `mq_parse_http`. The record gives no finer root cause. A Mongoose pull request says a libFuzzer harness on `mg_parse_http` found a memory-corruption bug. | https://cveawg.mitre.org/api/cve/CVE-2019-13503 (2019-07-11); https://github.com/cesanta/mongoose/pull/1035 (2019-07-07) |
| Buffer over-read | CVE-2026-69244 / GHSA-cq5v-8q36-5273 (aiohttp C response parser) | Out-of-bounds heap read in the C parser while building an error message for a malformed (chunked) response. Pure-Python parser unaffected. | https://api.github.com/advisories/GHSA-cq5v-8q36-5273 (2026-08-03) |
| Buffer over-read | CVE-2026-90588 (Zephyr HTTP client) | `print_header_field()` formats a parser callback slice with `%s`; the slice is not NUL terminated, so `strlen` reads past the receive buffer. The record text read was cut off, so the stated impact is unknown. | https://cveawg.mitre.org/api/cve/CVE-2026-90588 (2026-11-09) |
| Off-by-one | CVE-2023-51771 (MicroHttpServer) | `_ParseHeader` in `lib/server.c` allows a one byte overflow of the receive buffer with a long URI; the issue calls it an off-by-one global buffer overflow. | https://cveawg.mitre.org/api/cve/CVE-2023-51771 (2023-12-25); https://github.com/starnight/MicroHttpServer/issues/8 (2023-12-25) |
| Off-by-one | CVE-2025-30742 (atophttpd 2.8.0) | `httpd.c` off-by-one causing an out-of-bounds read, because a 1024 character request string gets no final NUL. A request-handling daemon rather than a reusable parser. | https://cveawg.mitre.org/api/cve/CVE-2025-30742 (2025-03-26) |
| Uninitialized memory (loose fit) | CVE-2026-60005 (nginx slice module) | Crafted requests with the slice directive and unnamed regex captures, or during a background cache update, cause uninitialized memory access in the worker (CWE-908). Not in the HTTP/1 parser itself. | https://cveawg.mitre.org/api/cve/CVE-2026-60005 (2026-07-15) |
| Uninitialized memory, in Rust | RUSTSEC-2022-0022 / GHSA-f67m-9j94-qv9j (hyper before 0.14.12) | The HTTP/1 parser used `mem::uninitialized()` to build `httparse::Header` values, which hold references and must not be null. Rated unsound; fixed with `MaybeUninit`. No CVE. | https://rustsec.org/advisories/RUSTSEC-2022-0022.html (2022-05-10); https://api.github.com/advisories/GHSA-f67m-9j94-qv9j (2022-06-16) |

Not used: no verified C HTTP/1 parser advisory for uninitialized memory in the parser proper
turned up, which is why the last two rows are the nearest fits (one a different component,
one a Rust example of the same mistake). A picohttpparser pull request (h2o/picohttpparser#95)
widened a `100 * 1024` constant that overflows `int` on 16-bit targets; it is a portability
fix, not a vulnerability, and is mentioned only because it is the same class as the
chunk-overhead arithmetic below.

## Class 1: buffer over-read

C pattern: a pointer is advanced and dereferenced after a check that was supposed to prove
there was a byte there. picohttpparser does this deliberately in places, for example it
requires nine bytes before judging `HTTP/1.x` so it can read the byte after the digit
without another end check, and `ADVANCE_TOKEN` relies on `CHECK_EOF` having run on every
path. One missed path is an over-read.

Rust: every read goes through `slice.get(i)` or an iterator over the remaining slice, and
"no more bytes" is `Status::Partial` through the `?` operator. There is no pointer to run
past the end. The seeded bug `MissingBoundsCheck` removes the guard on purpose: in C that is
a one-byte over-read, here it becomes an index panic that the harness catches as
`RustPanic` (see the README transcript). The shim allocates the input with exactly `len`
bytes, so under AddressSanitizer the C equivalent would be reported too.

In-library example: `is_complete` and `last_len` (DEVIATIONS.md, D2). A caller passing
`last_len` larger than the buffer makes the C code read out of bounds; the Rust API turns it
into `Error::InvalidLastLen`.

## Class 2: integer overflow in length arithmetic

C pattern: a size accumulated from untrusted digits (`size = size * 16 + digit`), a length
computed by subtracting pointers and returned as `int`, or a sum of lengths compared against
a limit.

picohttpparser limits the chunk size to `sizeof(size_t) * 2` hex digits so the accumulation
cannot wrap, and that guard is the only thing standing between a long size line and
CVE-2013-2028 style behaviour. In Rust the accumulation is `usize` arithmetic in a build
where overflow checks are on in debug and tests, the digit-count guard is kept, and the
copy is `copy_within(src..src + take, dst)` where `take` is `min(avail, bytes_left)`, so a
huge claimed chunk size can never cause an out-of-range copy: a bad range panics rather
than corrupting memory, and the fuzz target `rust_only` runs with debug assertions to find
such panics. The corpus has the boundary sizes (16 digits, 17 digits, 19 digits with
leading zeros, `7fffffffffffffff`, `ffffffffffffffff`) and the harness checks C and Rust
agree on each. The overhead guard (`total_read`, `total_overhead`) uses wrapping adds and
`wrapping_sub` to match the C unsigned arithmetic.

Not prevented by the language: the C `int` return truncation for heads above 2 GiB. The Rust
API returns `usize`, so it is avoided by type choice, not by anything Rust enforces.

## Class 3: off-by-one

C pattern: a length is `end - start` where one of them is off by one, or a loop bound is
`<=` where `<` was meant. These show up as a byte too many copied or a NUL written one
past the end.

Rust: slices carry their length and range indexing is checked, so an off-by-one is either
an `Option::None`/panic or, if the arithmetic is silently wrong, a wrong value rather than
memory corruption. The seeded bug `HeaderLenOffByOne` (header values over 16 bytes come back
one byte short) shows the second case: safe Rust does not stop a logic off-by-one, which is
why the differential harness is needed in addition to the language. The harness found it in
3 inputs.

## Class 4: use of uninitialized memory

C pattern: output parameters and arrays that are only partly written. picohttpparser fills
`phr_header` entries as it goes and the caller must trust `num_headers`; on failure the
out-parameters hold partial values. A caller that reads the array past `num_headers`, or
the method pointer after a failed parse, reads garbage.

Rust: there is no uninitialized safe memory. The caller's header slice is initialised by the
type system (`[Header::EMPTY; N]`), results are returned only on success, and `#![forbid(unsafe_code)]`
rules out `MaybeUninit` and `mem::uninitialized`. The hyper advisory above shows the way
this goes wrong when `unsafe` is used to avoid initialising: the bug came from
`unsafe` code used to skip initialisation, and this crate forbids `unsafe`.

## What the Rust version does not fix

Logic errors (class 3's logic variant above), parser differentials between this and another
HTTP implementation, denial of service by slow input (the `last_len` shortcut and the chunk
overhead guard are ported, nothing more), and anything in the C shim used by the harness.
The language removes the memory-corruption outcome, not the need to test behaviour.

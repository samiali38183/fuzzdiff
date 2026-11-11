# Benchmarks

Rust is slower than the C original at parsing heads, by roughly 1.4x to 1.5x with a reused
header array. On chunked decoding the two are level. These are one-machine numbers from a
single run, so treat differences under about 5% as noise.

## Setup

* CPU: AMD Ryzen 5 5600G (6 cores, 12 threads), 16 GB RAM, Windows 11 Home, nothing
  else deliberately running.
* Rust: rustc 1.99.1, `cargo bench` (release profile, opt-level 3, no `target-cpu`
  flags, default LTO setting).
* C: Microsoft cl 19.39.33523 (Visual Studio 2022 Build Tools), compiled by the `cc` crate with
  its release flags. MSVC does not define `__SSE4_2__`, so the C code ran its portable
  scalar path, not the SSE4.2 path. On a Linux/gcc build with `-msse4.2` the C side may be
  faster than shown here; that was not measured.
* Harness: criterion 0.5, 2 s warm-up, 5 s measurement per case, default sample count.
  Command: `cargo bench -p fuzzdiff-harness --bench throughput -- --warm-up-time 2 --measurement-time 5`.
* Both sides parse into a 64-entry header array and allocate nothing. The C side goes through
  `fd_bench_*` in `c/shim.c`, which calls the library directly on a stack array (no input
  copy). The call crosses an FFI boundary on the C side; the compiler cannot inline across it,
  and the same is not true of the Rust side. That slightly favours Rust.
* The chunked benchmark copies the input into a scratch buffer on both sides before decoding
  in place, so it includes one `memcpy` of the whole body.
* Inputs: `corpus/bench/`. The browser request is a Chrome-style GET with nine headers and a
  cookie, the API request is a POST with a bearer token, the response is a typical nginx
  200. The chunked inputs are 64 chunks of 1 KiB and 256 chunks of 16 bytes.

## Results (median of the criterion interval)

| Case | Bytes | C | Rust (reused header array) | Rust / C | Rust (fresh 64-entry array per call) |
|---|---|---|---|---|---|
| request, small GET | 82 | 42.3 ns (1.80 GiB/s) | 59.4 ns (1.29 GiB/s) | 1.40x | 88.1 ns |
| request, POST with token | 329 | 140.9 ns (2.17 GiB/s) | 205.2 ns (1.49 GiB/s) | 1.46x | 232.5 ns |
| request, browser | 484 | 207.3 ns (2.17 GiB/s) | 311.8 ns (1.45 GiB/s) | 1.50x | 317.6 ns |
| response, typical | 264 | 116.4 ns (2.11 GiB/s) | 169.4 ns (1.45 GiB/s) | 1.46x | not measured |
| chunked, 64 x 1 KiB | 65,989 | 2.606 us (23.6 GiB/s) | 2.573 us (23.9 GiB/s) | 0.99x | not applicable |
| chunked, 256 x 16 B | 5,637 | 2.026 us (2.59 GiB/s) | 2.038 us (2.58 GiB/s) | 1.01x | not applicable |

The chunked rows are within noise of each other (the Rust interval for the small-chunk case
is 1.89 to 2.22 us). Sizes are the file sizes in `corpus/bench/`.

## Reading the numbers

Rust is about 40 to 50% slower on head parsing. Filling the header array on every call
(a 64-entry array of 32-byte `Header` values, 2 KiB, is initialised before parsing) adds a further
10 to 45 ns, which is why the reused-array column is the fair comparison to C, where the
caller's array is not initialised.

Why the scalar head parser is slower was not profiled. Plausible causes, none verified: the
C code scans eight bytes per loop iteration with an unrolled check, while the Rust code runs a
`position` search with a closure that tests two conditions per byte, and each field is found
by a separate pass; header names go through a 256-entry bool table in both. If throughput on
heads mattered, the first thing to try would be a profile and a manual 8-byte scan, not a
design change. Speed was not a goal of this port; being identical to C was.

Reproduce: `make bench`. Criterion writes HTML reports to `target/criterion/`.

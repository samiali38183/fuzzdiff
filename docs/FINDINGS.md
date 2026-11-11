# Findings

Status on 2026-11-10: no divergence between the Rust port and the vendored C code has been
found, and no memory error in the C code has been observed.

Status on 2026-11-11, after the first successful CI runs: still no divergence and no memory
error observed. `ci` run 42961733968 and `fuzz` run 42961733885 are green: the C code ran
under ASan and UBSan during `cargo test` (portable and SSE4.2 variants), and the three
libFuzzer targets ran 120 s each (about 5.0M, 7.8M and 6.5M executions, see the README) without
a crash or a divergence. That is a short run; it lowers, but does not remove, the chance of a
finding.

What was run locally (Windows, MSVC build of the C code without sanitizers):

* the corpus replay and property tests in `harness/tests/`
* a 90 second run of the seeded generator: 27,639,000 inputs, 0 divergences

What was not run locally: libFuzzer, and the C code under AddressSanitizer or UBSan; those
ran only in CI, with the limits stated above. Longer runs have not been done.

One observation from reading the code, not from a run: the C `is_complete` reads out of
bounds when `last_len` is more than `len + 3`. That is a violation of the documented
calling convention rather than an input-triggered bug, and it is described in
DEVIATIONS.md (D2). The fuzz targets bound `last_len` to valid values, so none of the runs
above exercises it; it has not been reproduced under a sanitizer.

If a CI fuzz run produces a crash or a divergence, do not add the input or details to this
file or to the public repository before deciding whether it should be reported upstream.

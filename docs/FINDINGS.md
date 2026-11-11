# Findings

Status on 2026-11-10: no divergence between the Rust port and the vendored C code has been
found, and no memory error in the C code has been observed.

What was run (all locally, Windows, MSVC build of the C code without sanitizers):

* the corpus replay and property tests in `harness/tests/`
* a 90 second run of the seeded generator: 27,639,000 inputs, 0 divergences

What was not run: libFuzzer, and the C code under AddressSanitizer or UBSan. A memory error
in the C code would mostly not show up in the local runs, because they have no sanitizer.
The absence of findings here is therefore weak evidence about the C code, and says more
about the Rust port matching it on the inputs tried.

One observation from reading the code, not from a run: the C `is_complete` reads out of
bounds when `last_len` is more than `len + 3`. That is a violation of the documented
calling convention rather than an input-triggered bug, and it is described in
DEVIATIONS.md (D2). It has not been reproduced under a sanitizer.

If a CI fuzz run produces a crash or a divergence, do not add the input or details to this
file or to the public repository before deciding whether it should be reported upstream.

//! Window-boundary sweep for the C parser's 16-byte SIMD scans (see `simd_boundary_cases`).
//!
//! In a plain `cargo test` this exercises whichever C path the build selected. CI runs it with
//! the portable path and with `FUZZDIFF_C_SSE42=1`, under ASan and UBSan on Linux, where the
//! exact-size heap copies in the shim turn a 16-byte load past the end into a report.

use fuzzdiff_harness::generate::{simd_boundary_cases, SWEEP_MAX_FIELD_LEN};
use fuzzdiff_harness::{compare_case, describe, Outcome};

#[test]
fn every_byte_at_every_offset_of_every_block_agrees_with_c() {
    let (mut equal, mut reject, mut stricter) = (0usize, 0usize, 0usize);
    let total = simd_boundary_cases(|field, case| match compare_case(&case) {
        Outcome::Equal => equal += 1,
        Outcome::BothReject => reject += 1,
        Outcome::RustStricter(_) => stricter += 1,
        Outcome::Divergence(d) => panic!(
            "{field:?}: {:?}\n{}\n{}",
            d.kind,
            d.detail,
            describe(&case).chars().take(300).collect::<String>()
        ),
    });
    assert_eq!(total, equal + reject + stricter);
    // Guard against the sweep silently shrinking.
    assert_eq!(
        total,
        5 * 2 * 256 * (1 + (1..=SWEEP_MAX_FIELD_LEN).sum::<usize>())
    );
    // Both verdicts must occur in volume, or the sweep is not testing what it should.
    assert!(equal > 100_000 && reject > 100_000, "{equal} {reject}");
    println!("sweep: {total} inputs, equal={equal} both_reject={reject} rust_stricter={stricter}");
}

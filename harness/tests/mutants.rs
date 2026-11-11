//! Proves the differential harness can catch bugs: each seeded mutant in the
//! Rust parser must be flagged as a Divergence, and the unmutated parser must not.
//!
//! Run with: cargo test -p fuzzdiff-harness --features mutants --release --test mutants -- --nocapture
#![cfg(feature = "mutants")]

use fuzzdiff::mutants::{set_active, Mutant};
use fuzzdiff_harness::generate::{random_case, Rng};
use fuzzdiff_harness::{compare_case, describe, DivergenceKind, Outcome};
use std::time::Instant;

const BUDGET: u64 = 2_000_000;

#[test]
fn harness_detects_every_seeded_bug() {
    // Mutant M2 panics by design; keep the output readable.
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    println!("\nmutant detection (deterministic generator, seed = input index + 1)");
    for m in Mutant::ALL {
        set_active(Some(m));
        let start = Instant::now();
        let mut found = None;
        for i in 0..BUDGET {
            let case = random_case(&mut Rng::new(i + 1));
            if let Outcome::Divergence(d) = compare_case(&case) {
                found = Some((i + 1, d, case));
                break;
            }
        }
        set_active(None);
        let (n, d, case) = found.unwrap_or_else(|| panic!("{m:?} not detected in {BUDGET} inputs"));
        println!(
            "{m:?}: caught after {n} inputs in {:?} as {:?}",
            start.elapsed(),
            d.kind
        );
        println!("  first failing input: {}", describe(&case));
        if m == Mutant::MissingBoundsCheck {
            assert_eq!(d.kind, DivergenceKind::RustPanic);
        } else {
            assert_eq!(d.kind, DivergenceKind::ResultMismatch);
        }
    }

    // Control: same generator, no mutant active, must find nothing.
    set_active(None);
    let start = Instant::now();
    for i in 0..BUDGET / 4 {
        let case = random_case(&mut Rng::new(i + 1));
        let o = compare_case(&case);
        assert!(
            !o.is_divergence(),
            "control run diverged: {o:?}\n{}",
            describe(&case)
        );
    }
    println!(
        "control (no mutant): {} inputs, no divergence, {:?}",
        BUDGET / 4,
        start.elapsed()
    );
    std::panic::set_hook(old_hook);
}

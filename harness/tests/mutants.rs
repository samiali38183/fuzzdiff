//! Proves the differential harness can catch bugs: each seeded mutant in the
//! Rust parser must be flagged as a Divergence, and the unmutated parser must not.
//!
//! Each mutant is looked for in two ways: the deterministic random generator (the numbers
//! are "inputs until detection") and the hand-built `targeted_cases`, for guards the
//! generator cannot reach. A mutant must be caught by at least one of them.
//!
//! Run with: cargo test -p fuzzdiff-harness --features mutants --release --test mutants -- --nocapture
#![cfg(feature = "mutants")]

use fuzzdiff::mutants::{set_active, Mutant};
use fuzzdiff_harness::generate::{random_case, targeted_cases, Rng};
use fuzzdiff_harness::{compare_case, describe, DivergenceKind, Outcome};
use std::time::Instant;

const BUDGET: u64 = 2_000_000;

/// Mutants the random generator is expected to find (the other two need targeted cases).
const GENERATOR_REACHES: [Mutant; 3] = [
    Mutant::HeaderLenOffByOne,
    Mutant::MissingBoundsCheck,
    Mutant::WrongStateAfterCrlf,
];

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
        let generator_time = start.elapsed();
        let targeted =
            targeted_cases()
                .into_iter()
                .enumerate()
                .find_map(|(i, c)| match compare_case(&c) {
                    Outcome::Divergence(d) => Some((i + 1, d, c)),
                    _ => None,
                });
        set_active(None);

        match &found {
            Some((n, d, _)) => println!(
                "{m:?}: generator caught after {n} inputs in {generator_time:?} as {:?}",
                d.kind
            ),
            None => println!(
                "{m:?}: generator did not catch it in {BUDGET} inputs ({generator_time:?})"
            ),
        }
        match &targeted {
            Some((n, d, c)) => println!(
                "{m:?}: targeted case #{n} catches it as {:?}: {}",
                d.kind,
                describe(c).chars().take(120).collect::<String>()
            ),
            None => println!("{m:?}: no targeted case catches it"),
        }
        assert!(
            found.is_some() || targeted.is_some(),
            "{m:?} not detected by the generator or by the targeted cases"
        );
        if GENERATOR_REACHES.contains(&m) {
            let (_, d, _) = found.unwrap_or_else(|| panic!("{m:?} not found by the generator"));
            if m == Mutant::MissingBoundsCheck {
                assert_eq!(d.kind, DivergenceKind::RustPanic);
            } else {
                assert_eq!(d.kind, DivergenceKind::ResultMismatch);
            }
        } else {
            assert!(
                targeted.is_some(),
                "{m:?} should be caught by a targeted case"
            );
        }
    }

    // Control: same generator and targeted cases, no mutant active, must find nothing.
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
    for c in targeted_cases() {
        assert!(!compare_case(&c).is_divergence());
    }
    println!(
        "control (no mutant): {} inputs and {} targeted cases, no divergence, {:?}",
        BUDGET / 4,
        targeted_cases().len(),
        start.elapsed()
    );
    std::panic::set_hook(old_hook);
}

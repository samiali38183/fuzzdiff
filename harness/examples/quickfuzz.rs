//! Local stand-in for the CI fuzz job: runs the seeded generator against both
//! implementations for a fixed time and reports any divergence.
//!
//! cargo run --release -p fuzzdiff-harness --example quickfuzz -- 60 [first-seed]

use fuzzdiff_harness::generate::{random_case, Rng};
use fuzzdiff_harness::{compare_case, describe, Outcome};
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let mut seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let end = Instant::now() + Duration::from_secs(secs);
    let (mut n, mut equal, mut both_reject, mut stricter) = (0u64, 0u64, 0u64, 0u64);
    while Instant::now() < end {
        for _ in 0..1000 {
            let case = random_case(&mut Rng::new(seed));
            match compare_case(&case) {
                Outcome::Equal => equal += 1,
                Outcome::BothReject => both_reject += 1,
                Outcome::RustStricter(_) => stricter += 1,
                Outcome::Divergence(d) => {
                    println!(
                        "DIVERGENCE seed={seed} kind={:?}\n{}\n{}",
                        d.kind,
                        d.detail,
                        describe(&case)
                    );
                    std::process::exit(1);
                }
            }
            seed += 1;
            n += 1;
        }
    }
    println!("{n} inputs in {secs}s (seeds up to {seed}): equal={equal} both_reject={both_reject} rust_stricter={stricter} divergences=0");
}

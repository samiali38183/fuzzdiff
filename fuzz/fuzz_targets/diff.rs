//! Differential target: C and Rust must agree. Documented deviations (RustStricter) are fine.
#![no_main]

use fuzzdiff_harness::{compare, Outcome};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Outcome::Divergence(d) = compare(data) {
        panic!("{:?}: {}", d.kind, d.detail);
    }
});

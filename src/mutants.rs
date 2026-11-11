//! Deliberate bugs for testing the test harness.
//!
//! With the `mutants` cargo feature on, [`set_active`] switches one of five
//! known-bad behaviours on for the current thread. The differential harness
//! must detect each of them. Without the feature everything here compiles to
//! a constant `false`, so the shipped parser contains none of this code.

/// A seeded bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mutant {
    /// Header values longer than 16 bytes come back one byte short (an
    /// off-by-one in the length, the kind of bug a SIMD block boundary causes).
    HeaderLenOffByOne,
    /// The end-of-buffer guard before the byte after `HTTP/1.x` is missing,
    /// so a head cut off right after the version reads one byte too far. In C
    /// that is an over-read; in safe Rust it is an index panic.
    MissingBoundsCheck,
    /// After a trailer line's CRLF the chunked decoder goes to the wrong
    /// state, so the blank line that ends the trailers is never recognised.
    WrongStateAfterCrlf,
    /// The chunked decoder's framing-overhead guard uses `/ 5` instead of `/ 4`, so it fires
    /// at 80% overhead instead of 75%. Only inputs with more than 100 KiB of framing
    /// reach it, which the random generator never builds.
    OverheadRatio,
    /// The response parser waits for 3 bytes after the status-line spaces instead of 4,
    /// so a head cut right after a malformed status code is rejected instead of "partial".
    StatusGuard,
}

impl Mutant {
    /// All seeded bugs.
    pub const ALL: [Mutant; 5] = [
        Mutant::HeaderLenOffByOne,
        Mutant::MissingBoundsCheck,
        Mutant::WrongStateAfterCrlf,
        Mutant::OverheadRatio,
        Mutant::StatusGuard,
    ];
}

#[cfg(feature = "mutants")]
mod imp {
    use super::Mutant;
    use std::cell::Cell;

    thread_local! {
        static ACTIVE: Cell<Option<Mutant>> = const { Cell::new(None) };
    }

    /// Select the active mutant for the current thread (`None` = correct parser).
    pub fn set_active(m: Option<Mutant>) {
        ACTIVE.with(|a| a.set(m));
    }

    #[inline]
    pub(crate) fn active(m: Mutant) -> bool {
        ACTIVE.with(|a| a.get() == Some(m))
    }
}

#[cfg(feature = "mutants")]
pub use imp::set_active;

#[cfg(feature = "mutants")]
pub(crate) use imp::active;

#[cfg(not(feature = "mutants"))]
#[inline(always)]
pub(crate) const fn active(_m: Mutant) -> bool {
    false
}

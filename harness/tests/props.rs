//! Property tests: for generated and mutated inputs the C and Rust parsers agree,
//! and the Rust side never panics.

use fuzzdiff_harness::generate::{random_case, Rng};
use fuzzdiff_harness::{compare, compare_case, describe, Case, Op, Outcome};
use proptest::prelude::*;

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000)
}

fn cfg() -> ProptestConfig {
    ProptestConfig {
        cases: cases(),
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(cfg())]

    /// Whole-case generator (valid or mutated message, random parameters).
    #[test]
    fn generated_cases_agree(seed in any::<u64>()) {
        let case = random_case(&mut Rng::new(seed));
        let o = compare_case(&case);
        prop_assert!(!o.is_divergence(), "{:?}\n{}", o, describe(&case));
    }

    /// Raw bytes through the fuzzer entry point, so the control-byte decoding is covered too.
    #[test]
    fn raw_bytes_agree(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
        let o = compare(&bytes);
        prop_assert!(!o.is_divergence(), "{:?}", o);
    }

    /// Bytes drawn from the HTTP-significant alphabet reach deep parser states
    /// and shrink to readable counterexamples.
    #[test]
    fn http_alphabet_agrees(
        op in 0usize..4,
        data in proptest::collection::vec(
            prop_oneof![
                Just(b'\r'), Just(b'\n'), Just(b' '), Just(b'\t'), Just(b':'), Just(b';'),
                Just(b'0'), Just(b'1'), Just(b'f'), Just(b'H'), Just(b'T'), Just(b'P'),
                Just(b'/'), Just(b'.'), Just(b'G'), Just(b'a'), Just(0u8), Just(0x7fu8), Just(0xffu8),
            ],
            0..120,
        ),
        last_len_frac in 0usize..4,
        max_headers in 0usize..6,
        piece in 0usize..5,
        strict in any::<bool>(),
        trailer in any::<bool>(),
    ) {
        let op = [Op::Request, Op::Response, Op::Headers, Op::Chunked][op];
        let mut c = Case::new(op, data.clone()).with_piece_size(piece);
        c.last_len = data.len() * last_len_frac / 3;
        c.max_headers = max_headers;
        c.strict = strict;
        c.consume_trailer = trailer;
        let o = compare_case(&c);
        prop_assert!(!o.is_divergence(), "{:?}\n{}", o, describe(&c));
    }

    /// Splitting a chunked body anywhere never changes the decoded result
    /// (checked against C for each split, and Rust is compared to C).
    #[test]
    fn chunked_split_invariance(seed in any::<u64>(), cut in 0usize..200, trailer in any::<bool>()) {
        let mut r = Rng::new(seed);
        let data = fuzzdiff_harness::generate::valid_chunked(&mut r);
        let cut = cut.min(data.len());
        let mut c = Case::new(Op::Chunked, data.clone());
        c.consume_trailer = trailer;
        c.pieces = vec![cut, data.len() - cut];
        let o = compare_case(&c);
        prop_assert!(matches!(o, Outcome::Equal | Outcome::BothReject), "{:?}", o);
    }

    /// Well-formed requests are parsed identically by both implementations.
    #[test]
    fn valid_requests_agree(seed in any::<u64>()) {
        let data = fuzzdiff_harness::generate::valid_request(&mut Rng::new(seed));
        let mut c = Case::new(Op::Request, data);
        c.max_headers = 64;
        let o = compare_case(&c);
        prop_assert!(!o.is_divergence(), "{:?}", o);
    }
}

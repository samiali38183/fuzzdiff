//! Cases aimed at guards that random generation does not reach (see `targeted_cases`).
//! With the parser unmodified they must show no divergence; the mutant test shows that
//! with the matching mutant switched on they do.

use fuzzdiff_harness::generate::targeted_cases;
use fuzzdiff_harness::{compare_case, describe, Outcome};

#[test]
fn targeted_cases_agree_with_c() {
    for c in targeted_cases() {
        let o = compare_case(&c);
        assert!(
            !o.is_divergence(),
            "{o:?}\n{}",
            describe(&c).chars().take(200).collect::<String>()
        );
    }
}

#[test]
fn overhead_guard_fires_between_75_and_80_percent() {
    let cases = targeted_cases();
    // 76.9% framing: both implementations must refuse (BothReject), not wait.
    assert_eq!(compare_case(&cases[0]), Outcome::BothReject);
    // 71.4%: still incomplete in both.
    assert_eq!(compare_case(&cases[1]), Outcome::Equal);
    // 90.9%: refused.
    assert_eq!(compare_case(&cases[2]), Outcome::BothReject);
}

#[test]
fn status_line_with_three_bytes_after_spaces_is_partial() {
    // The guard comes before the digit check, so a malformed code is "need more", not an error.
    for head in [&b"HTTP/1.1 2x1"[..], b"HTTP/1.1 20x", b"HTTP/1.1 x00"] {
        let mut h = [fuzzdiff::Header::EMPTY; 4];
        assert_eq!(
            fuzzdiff::parse_response(head, &mut h, 0),
            Ok(fuzzdiff::Status::Partial),
            "{}",
            String::from_utf8_lossy(head)
        );
    }
}

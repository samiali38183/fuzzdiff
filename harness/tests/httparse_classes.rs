//! Pins the disagreement classes between picohttpparser, fuzzdiff and httparse.
//! See docs/DIFFERENTIAL_HTTPARSE.md.

use fuzzdiff_harness::httparse_cases::EXAMPLES;
use fuzzdiff_harness::httparse_oracle::FEATURES;
use fuzzdiff_harness::httparse_report::{load_corpus, render_curated, sweep};
use std::path::PathBuf;

#[test]
fn curated_examples_have_the_pinned_patterns() {
    let mut bad = Vec::new();
    for e in EXAMPLES {
        let got = e.run().pattern();
        if got != e.expect {
            bad.push(format!(
                "[{}] {}: expected {}, got {}",
                e.class, e.label, e.expect, got
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "disagreement pattern changed:\n{}",
        bad.join("\n")
    );
}

#[test]
fn fuzzdiff_always_equals_the_c_parser_on_the_curated_examples() {
    for e in EXAMPLES {
        let t = e.run();
        assert_eq!(t.c, t.rust, "{}", e.label);
        assert!(!t.field_diff.contains(&"c/rust"), "{}", e.label);
    }
}

#[test]
fn generated_curated_table_is_up_to_date() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/DIFFERENTIAL_HTTPARSE.md");
    let doc = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let a = "<!-- BEGIN GENERATED: curated -->\n\n";
    let b = "\n<!-- END GENERATED: curated -->";
    let start = doc.find(a).expect("begin marker") + a.len();
    let end = doc.find(b).expect("end marker");
    assert_eq!(
        doc[start..end].trim_end(),
        render_curated().trim_end(),
        "docs/DIFFERENTIAL_HTTPARSE.md is stale; run the httparse_table example with --write"
    );
}

const KNOWN_PATTERNS: &[(&str, &str)] = &[
    ("headers", "CCR"),
    ("headers", "PPR"),
    ("request", "CCR"),
    ("request", "PPR"),
    ("request", "RRC"),
    ("request", "RRP"),
    ("response", "CCC!"),
    ("response", "CCR"),
    ("response", "PPR"),
    ("response", "RRC"),
    ("response", "RRP"),
];

const KNOWN_FEATURES: &[&str] = &[
    "obs-fold",
    "multi-space-start-line",
    "version-digits",
    "high-byte-start-line",
    "bare-lf",
    "bare-cr",
    "leading-empty-lines",
    "header-line-without-colon",
    "status-code-range",
];

#[test]
fn sweep_finds_no_new_disagreement_class() {
    let st = sweep(50_000, &load_corpus());
    assert_eq!(st.c_vs_rust, 0, "picohttpparser and fuzzdiff differ");
    assert_eq!(
        st.unclassified,
        0,
        "a disagreement no known feature explains, e.g. {:?}",
        st.unclassified_examples
            .iter()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect::<Vec<_>>()
    );
    for (op, pat) in st.patterns.keys() {
        assert!(
            KNOWN_PATTERNS.contains(&(*op, pat.as_str())),
            "new outcome pattern {pat} for {op}"
        );
    }
    for f in st.features.keys() {
        assert!(FEATURES.contains(f));
        assert!(KNOWN_FEATURES.contains(f), "new attributed feature {f}");
    }
    assert!(
        st.disagree() > 0,
        "sweep found no disagreement at all; the oracle is broken"
    );
}

#[test]
fn httparse_ends_a_chunk_line_with_bare_lf_inside_an_extension_at_the_next_crlf() {
    match httparse::parse_chunk_size(b"5;a=b\nhello\r\n0\r\n\r\n") {
        Ok(httparse::Status::Complete((n, size))) => {
            assert_eq!((n, size), (13, 5));
        }
        other => panic!("unexpected: {other:?}"),
    }
    // Without the extension a bare LF is an error.
    assert!(httparse::parse_chunk_size(b"5\nhello\r\n").is_err());
}

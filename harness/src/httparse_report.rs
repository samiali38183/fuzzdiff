//! Deterministic sweep and Markdown rendering for the three-way comparison.
//!
//! `cargo run --release -p fuzzdiff-harness --example httparse_table` prints the tables that
//! live in `docs/DIFFERENTIAL_HTTPARSE.md`; with `--write` it rewrites them in place, with
//! `--check` it fails if the committed file is out of date.

use crate::generate::{mutate, valid_headers, valid_request, valid_response, Rng};
use crate::httparse_cases::{Example, CLASSES, EXAMPLES};
use crate::httparse_oracle::{explain, tri_head, Explanation, Verdict, FEATURES};
use crate::Op;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

/// Counters for one feature.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FeatureCount {
    /// Disagreeing inputs attributed to the feature (see [`explain`]).
    pub any: u64,
    /// Of those, inputs where this feature alone explains the disagreement.
    pub only: u64,
    /// Per outcome pattern, among the attributed inputs.
    pub patterns: BTreeMap<String, u64>,
}

/// Result of [`sweep`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sweep {
    /// Inputs run.
    pub total: u64,
    /// Inputs per entry point.
    pub by_op: BTreeMap<&'static str, u64>,
    /// Inputs on which all three parsers did the same thing (including all three rejecting).
    pub agree: u64,
    /// Disagreeing inputs per `(entry point, pattern)`.
    pub patterns: BTreeMap<(&'static str, String), u64>,
    /// Disagreeing inputs per feature.
    pub features: BTreeMap<&'static str, FeatureCount>,
    /// Disagreements explained by no feature and not early rejection. Must be zero: a
    /// non-zero value means a disagreement class this crate does not know about.
    pub unclassified: u64,
    /// picohttpparser and fuzzdiff partial, httparse reject, no feature involved.
    pub early_reject: u64,
    /// More header lines than the shared 64-slot capacity (differing cap timing).
    pub header_cap: u64,
    /// A sample of those, for inspection.
    pub early_examples: Vec<(Op, Vec<u8>)>,
    /// The shortest few unclassified inputs.
    pub unclassified_examples: Vec<Vec<u8>>,
    /// Inputs where picohttpparser and fuzzdiff differ (a bug in one of them). Must be zero.
    pub c_vs_rust: u64,
    /// Inputs where all three completed but returned different fields.
    pub field_diff: u64,
}

fn i_sample(n: u64) -> bool {
    n % 3000 == 1
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::Request => "request",
        Op::Response => "response",
        Op::Headers => "headers",
        Op::Chunked => "chunked",
    }
}

/// The seed corpus files usable as bases for head parsing: `(op, bytes)`.
pub fn load_corpus() -> Vec<(Op, Vec<u8>)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus");
    let mut out = Vec::new();
    for (dir, op) in [
        ("request", Op::Request),
        ("response", Op::Response),
        ("headers", Op::Headers),
    ] {
        let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut files: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
        files.sort();
        for p in files {
            if let Ok(b) = std::fs::read(&p) {
                out.push((op, b));
            }
        }
    }
    out
}

/// Run `n` deterministic inputs (seed = index + 1) through the three parsers: half start from
/// a corpus file, half from the well-formed generators, two thirds are then mutated.
pub fn sweep(n: u64, corpus: &[(Op, Vec<u8>)]) -> Sweep {
    let mut st = Sweep::default();
    for i in 0..n {
        let mut r = Rng::new(i + 1);
        let op = [Op::Request, Op::Request, Op::Response, Op::Headers][r.below(4)];
        let pool: Vec<&Vec<u8>> = corpus
            .iter()
            .filter(|(o, _)| *o == op)
            .map(|(_, b)| b)
            .collect();
        let mut data = if !pool.is_empty() && r.chance(1, 2) {
            pool[r.below(pool.len())].clone()
        } else {
            match op {
                Op::Request => valid_request(&mut r),
                Op::Response => valid_response(&mut r),
                _ => valid_headers(&mut r),
            }
        };
        if r.chance(2, 3) {
            mutate(&mut r, &mut data);
        }
        st.record(op, &data);
    }
    st
}

impl Sweep {
    fn record(&mut self, op: Op, data: &[u8]) {
        self.total += 1;
        *self.by_op.entry(op_name(op)).or_default() += 1;
        let t = tri_head(op, data);
        if t.c != t.rust || t.field_diff.contains(&"c/rust") {
            self.c_vs_rust += 1;
        }
        if t.agrees() {
            self.agree += 1;
            return;
        }
        if t.field_diff.iter().any(|d| d.ends_with("/hp")) {
            self.field_diff += 1;
        }
        let pat = t.pattern();
        *self.patterns.entry((op_name(op), pat.clone())).or_default() += 1;
        match explain(op, data) {
            Explanation::Features(set) => {
                for name in &set {
                    let c = self.features.entry(*name).or_default();
                    c.any += 1;
                    if set.len() == 1 {
                        c.only += 1;
                    }
                    *c.patterns.entry(pat.clone()).or_default() += 1;
                }
            }
            Explanation::HeaderCap => self.header_cap += 1,
            Explanation::EarlyReject => {
                self.early_reject += 1;
                if self.early_examples.len() < 12 && i_sample(self.early_reject) {
                    self.early_examples.push((op, data.to_vec()));
                }
            }
            Explanation::Unexplained => {
                self.unclassified += 1;
                self.unclassified_examples.push(data.to_vec());
                self.unclassified_examples.sort_by_key(Vec::len);
                self.unclassified_examples.truncate(5);
            }
        }
    }

    /// Disagreeing inputs (all three did not do the same thing).
    pub fn disagree(&self) -> u64 {
        self.total - self.agree
    }
}

fn esc(b: &[u8]) -> String {
    let mut s = String::new();
    for &c in b {
        for e in std::ascii::escape_default(c) {
            s.push(char::from(e));
        }
    }
    s
}

fn word(v: Verdict) -> &'static str {
    match v {
        Verdict::Complete => "complete",
        Verdict::Partial => "partial",
        Verdict::Reject => "reject",
    }
}

fn stricter(e: &Example) -> &'static str {
    let t = e.run();
    if t.agrees() {
        return "same";
    }
    if t.c == Verdict::Complete && t.hp == Verdict::Reject {
        "httparse"
    } else if t.c == Verdict::Reject && t.hp == Verdict::Complete {
        "picohttpparser"
    } else if t.c == Verdict::Partial && t.hp == Verdict::Reject {
        "httparse (rejects earlier)"
    } else if t.c == t.hp && !t.field_diff.is_empty() {
        "same verdict, fields differ"
    } else {
        "different"
    }
}

/// The curated examples as a Markdown table.
pub fn render_curated() -> String {
    let mut s = String::new();
    s.push_str(
        "| Class | Example | picohttpparser | fuzzdiff | httparse | Pattern | Stricter |\n\
         |---|---|---|---|---|---|---|\n",
    );
    for (id, title, _) in CLASSES {
        for e in EXAMPLES.iter().filter(|e| e.class == *id) {
            let t = e.run();
            let _ = writeln!(
                s,
                "| {title} | `{}` ({}) | {} | {} | {} | {} | {} |",
                esc(e.bytes),
                e.label,
                word(t.c),
                word(t.rust),
                word(t.hp),
                t.pattern(),
                stricter(e)
            );
        }
    }
    s
}

/// The sweep result as Markdown tables.
pub fn render_sweep(st: &Sweep) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "Inputs: {} ({} request, {} response, {} header block). All three parsers did the same \
         thing on {} ({:.2}%); they disagreed on {}.\n",
        st.total,
        st.by_op.get("request").copied().unwrap_or(0),
        st.by_op.get("response").copied().unwrap_or(0),
        st.by_op.get("headers").copied().unwrap_or(0),
        st.agree,
        100.0 * st.agree as f64 / st.total.max(1) as f64,
        st.disagree()
    );
    let _ = writeln!(
        s,
        "picohttpparser and fuzzdiff differed on {} inputs (a non-zero number would be a bug \
         and fail the tests). Disagreements explained by no known feature: {}. Disagreements \
         that are only httparse rejecting an invalid prefix earlier: {}.\n",
        st.c_vs_rust, st.unclassified, st.early_reject
    );
    s.push_str("Disagreements by entry point and outcome pattern (picohttpparser, fuzzdiff, httparse; `C` complete, `P` partial, `R` reject, `!` all complete but fields differ):\n\n");
    s.push_str("| Entry point | Pattern | Inputs |\n|---|---|---|\n");
    for ((op, pat), n) in &st.patterns {
        let _ = writeln!(s, "| {op} | {pat} | {n} |");
    }
    s.push_str("\nDisagreements by attributed feature. Each disagreeing input is attributed to the smallest set of features whose removal (a rewrite of just that feature) makes all three parsers agree; an input attributed to two features counts under both, and \"alone\" counts the inputs where this feature is the whole explanation:\n\n");
    s.push_str("| Feature | Inputs attributed | Alone | Patterns (count) |\n|---|---|---|---|\n");
    for name in FEATURES {
        if let Some(c) = st.features.get(name) {
            let pats = c
                .patterns
                .iter()
                .map(|(p, n)| format!("{p} {n}"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(s, "| {name} | {} | {} | {pats} |", c.any, c.only);
        }
    }
    s
}

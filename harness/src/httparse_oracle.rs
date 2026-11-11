//! Third oracle: the `httparse` crate (the HTTP/1.x parser used by hyper).
//!
//! [`tri_head`] runs one input through picohttpparser (C, via the shim), the
//! fuzzdiff port and `httparse` and reports whether the three agree. The point is
//! not to find bugs in any of them (the first two are already compared by
//! [`crate::compare_case`], which must report no divergence) but to map where
//! "matches picohttpparser" and "matches a production Rust parser" are different
//! statements. Every disagreement between picohttpparser and httparse is a
//! documented leniency choice, listed in `docs/DIFFERENTIAL_HTTPARSE.md`.
//!
//! The fuzzdiff side always runs with the default `Config` (no `reject_obs_fold`),
//! which is the configuration that is byte-for-byte equivalent to the C library.

use crate::{c_head, rust_head, Case, Head, Norm, Op};
use fuzzdiff::Config;

/// Coarse outcome of one parser on one input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// A complete head was parsed.
    Complete,
    /// More bytes are needed.
    Partial,
    /// The input was rejected.
    Reject,
}

impl Verdict {
    fn letter(self) -> char {
        match self {
            Verdict::Complete => 'C',
            Verdict::Partial => 'P',
            Verdict::Reject => 'R',
        }
    }

    fn of(n: &Norm) -> Verdict {
        match n {
            Norm::Partial => Verdict::Partial,
            Norm::Reject | Norm::Other(_) => Verdict::Reject,
            Norm::Done(_) => Verdict::Complete,
        }
    }
}

/// What the three parsers did with one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triple {
    /// picohttpparser (C).
    pub c: Verdict,
    /// fuzzdiff (Rust port, default config).
    pub rust: Verdict,
    /// httparse 1.10.1.
    pub hp: Verdict,
    /// When all three completed: which pairs returned different fields
    /// (`"c/hp"`, `"rust/hp"`, `"c/rust"`), empty when all fields are equal.
    pub field_diff: Vec<&'static str>,
}

impl Triple {
    /// `true` when the three parsers did the same thing with the input.
    pub fn agrees(&self) -> bool {
        self.c == self.rust && self.rust == self.hp && self.field_diff.is_empty()
    }

    /// Stable short name of the outcome pattern, in the order (picohttpparser,
    /// fuzzdiff, httparse): `C` complete, `P` partial, `R` reject. A suffix `!`
    /// means all three completed but returned different fields.
    pub fn pattern(&self) -> String {
        let mut s = format!(
            "{}{}{}",
            self.c.letter(),
            self.rust.letter(),
            self.hp.letter()
        );
        if !self.field_diff.is_empty() {
            s.push('!');
        }
        s
    }
}

fn hp_head(op: Op, data: &[u8]) -> Norm {
    // Same capacity as the C and Rust sides in `tri_head`.
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let to_headers = |h: &[httparse::Header<'_>]| -> Vec<(Option<Vec<u8>>, Vec<u8>)> {
        h.iter()
            .map(|h| (Some(h.name.as_bytes().to_vec()), h.value.to_vec()))
            .collect()
    };
    match op {
        Op::Request => {
            let mut r = httparse::Request::new(&mut headers);
            match r.parse(data) {
                Ok(httparse::Status::Partial) => Norm::Partial,
                Ok(httparse::Status::Complete(n)) => Norm::Done(Box::new(Head {
                    consumed: n,
                    method: r.method.unwrap_or("").as_bytes().to_vec(),
                    path: r.path.unwrap_or("").as_bytes().to_vec(),
                    minor_version: i32::from(r.version.unwrap_or(0)),
                    headers: to_headers(r.headers),
                    ..Head::default()
                })),
                Err(_) => Norm::Reject,
            }
        }
        Op::Response => {
            let mut r = httparse::Response::new(&mut headers);
            match r.parse(data) {
                Ok(httparse::Status::Partial) => Norm::Partial,
                Ok(httparse::Status::Complete(n)) => Norm::Done(Box::new(Head {
                    consumed: n,
                    reason: r.reason.unwrap_or("").as_bytes().to_vec(),
                    minor_version: i32::from(r.version.unwrap_or(0)),
                    status: i32::from(r.code.unwrap_or(0)),
                    headers: to_headers(r.headers),
                    ..Head::default()
                })),
                Err(_) => Norm::Reject,
            }
        }
        _ => match httparse::parse_headers(data, &mut headers) {
            Ok(httparse::Status::Partial) => Norm::Partial,
            Ok(httparse::Status::Complete((n, h))) => Norm::Done(Box::new(Head {
                consumed: n,
                headers: to_headers(h),
                ..Head::default()
            })),
            Err(_) => Norm::Reject,
        },
    }
}

/// Run a head parse (`Op::Request`, `Op::Response` or `Op::Headers`) through all
/// three parsers. Chunked bodies are handled by [`tri_chunk_size`].
///
/// # Panics
/// If `op` is `Op::Chunked`.
pub fn tri_head(op: Op, data: &[u8]) -> Triple {
    assert!(op != Op::Chunked, "use tri_chunk_size for chunked input");
    let mut case = Case::new(op, data.to_vec());
    case.max_headers = 64;
    let c = c_head(&case).unwrap_or(Norm::Reject);
    let rust = rust_head(&case, Config::default()).0;
    let hp = hp_head(op, data);
    let mut field_diff = Vec::new();
    if let (Norm::Done(a), Norm::Done(b), Norm::Done(h)) = (&c, &rust, &hp) {
        // httparse always has a name; picohttpparser reports a folded line as a nameless header.
        if a != h {
            field_diff.push("c/hp");
        }
        if b != h {
            field_diff.push("rust/hp");
        }
        if a != b {
            field_diff.push("c/rust");
        }
    }
    Triple {
        c: Verdict::of(&c),
        rust: Verdict::of(&rust),
        hp: Verdict::of(&hp),
        field_diff,
    }
}

/// Run a chunked body through picohttpparser and fuzzdiff (whole body in one call)
/// and its first line, the chunk-size line with any extension, through
/// `httparse::parse_chunk_size`. For the verdicts to be comparable the body after
/// that line must be well-formed, so that the decoder's result is decided by the
/// size line alone (the curated examples in this module's callers do that).
pub fn tri_chunk_size(body: &[u8]) -> Triple {
    let case = Case::new(Op::Chunked, body.to_vec());
    let c = crate::ffi::c_chunked(&case.data, &case.pieces, false);
    let r = crate::rust_chunked(&case);
    let verdict = |steps: &crate::ffi::CChunked| match steps.steps.last().map(|s| s.ret) {
        Some(-1) => Verdict::Reject,
        Some(-2) | None => Verdict::Partial,
        Some(_) => Verdict::Complete,
    };
    let hp = match httparse::parse_chunk_size(body) {
        Ok(httparse::Status::Complete(_)) => Verdict::Complete,
        Ok(httparse::Status::Partial) => Verdict::Partial,
        Err(_) => Verdict::Reject,
    };
    Triple {
        c: verdict(&c),
        rust: verdict(&r),
        hp,
        field_diff: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Attribution: which known leniency feature explains a disagreement.
//
// A feature being present in the bytes does not mean it caused a disagreement (most
// generated inputs contain high bytes, for example). So the attribution is causal: for
// each feature there is a neutralizer that rewrites the input to remove just that feature,
// and a disagreement is attributed to the smallest set of features whose neutralization
// makes the three parsers agree.

/// Names of the features, in the order used in the docs.
pub const FEATURES: [&str; 15] = [
    "obs-fold",
    "multi-space-start-line",
    "version-digits",
    "high-byte-start-line",
    "high-byte-header-value",
    "high-byte-header-name",
    "space-before-colon",
    "header-name-charset",
    "bare-lf",
    "bare-cr",
    "nul",
    "other-ctl",
    "leading-empty-lines",
    "header-line-without-colon",
    "status-code-range",
];

fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

fn strip_eol(line: &[u8]) -> &[u8] {
    let l = line.strip_suffix(b"\n").unwrap_or(line);
    l.strip_suffix(b"\r").unwrap_or(l)
}

fn is_empty_line(line: &[u8]) -> bool {
    strip_eol(line).is_empty() && !line.is_empty()
}

/// Which of the [`FEATURES`] the input contains. Looks at the bytes only, runs no parser.
pub fn features(op: Op, data: &[u8]) -> Vec<&'static str> {
    FEATURES
        .iter()
        .copied()
        .filter(|f| neutralize(op, data, f) != data)
        .collect()
}

/// Rewrite `data` so that the named feature is no longer present, changing as little else as
/// possible. Returns the input unchanged when the feature is absent.
pub fn neutralize(op: Op, data: &[u8], feature: &str) -> Vec<u8> {
    match feature {
        "bare-lf" => {
            let mut out = Vec::with_capacity(data.len() + 8);
            for (i, &b) in data.iter().enumerate() {
                if b == b'\n' && (i == 0 || data[i - 1] != b'\r') {
                    out.push(b'\r');
                }
                out.push(b);
            }
            return out;
        }
        "bare-cr" => {
            return data
                .iter()
                .enumerate()
                .filter(|&(i, &b)| !(b == b'\r' && data.get(i + 1) != Some(&b'\n')))
                .map(|(_, &b)| b)
                .collect();
        }
        "nul" => {
            return data
                .iter()
                .map(|&b| if b == 0 { b'x' } else { b })
                .collect()
        }
        "other-ctl" => {
            return data
                .iter()
                .map(|&b| {
                    if (b < 0x20 && b != b'\t' && b != b'\r' && b != b'\n') || b == 0x7f {
                        b'x'
                    } else {
                        b
                    }
                })
                .collect()
        }
        _ => {}
    }

    let lines: Vec<&[u8]> = data.split_inclusive(|&b| b == b'\n').collect();
    let skip = if op == Op::Headers {
        0
    } else {
        lines.iter().take_while(|l| is_empty_line(l)).count()
    };
    if feature == "leading-empty-lines" {
        return lines[skip..].concat();
    }
    let mut out: Vec<u8> = Vec::with_capacity(data.len());
    let mut in_head = true;
    for (i, line) in lines.iter().enumerate() {
        if i < skip || !in_head {
            out.extend_from_slice(line);
            continue;
        }
        if is_empty_line(line) {
            in_head = false;
            out.extend_from_slice(line);
            continue;
        }
        let first = op != Op::Headers && i == skip;
        let eol = &line[strip_eol(line).len()..];
        let text = strip_eol(line);
        if first {
            let mut t = text.to_vec();
            match feature {
                "multi-space-start-line" => {
                    t.dedup_by(|a, b| *a == b' ' && *b == b' ');
                }
                "version-digits" => {
                    // The version is the last "HTTP/" of a request line (the target can
                    // contain the same text), the first of a status line.
                    let at = if op == Op::Request {
                        t.windows(5).rposition(|w| w == b"HTTP/")
                    } else {
                        t.windows(5).position(|w| w == b"HTTP/")
                    };
                    if let Some(p) = at {
                        if t.len() >= p + 8 {
                            t[p + 5..p + 8].copy_from_slice(b"1.1");
                        }
                    }
                }
                "high-byte-start-line" => {
                    for b in &mut t {
                        if *b >= 0x80 {
                            *b = b'x';
                        }
                    }
                }
                "status-code-range" if op == Op::Response => {
                    if let Some(p) = t.iter().position(|&b| b == b' ') {
                        if matches!(t.get(p + 1), Some(b'0' | b'6'..=b'9')) {
                            t[p + 1] = b'2';
                        }
                    }
                }
                _ => {}
            }
            out.extend_from_slice(&t);
            out.extend_from_slice(eol);
            continue;
        }
        if text.first().is_some_and(|&b| b == b' ' || b == b'\t') {
            // A folded line.
            match feature {
                "obs-fold" if !out.is_empty() => {
                    while out.last().is_some_and(|&b| b == b'\n' || b == b'\r') {
                        out.pop();
                    }
                    out.push(b' ');
                    let t = text
                        .iter()
                        .position(|&b| b != b' ' && b != b'\t')
                        .map_or(&text[text.len()..], |p| &text[p..]);
                    out.extend_from_slice(t);
                    out.extend_from_slice(eol);
                }
                "high-byte-header-value" => {
                    out.extend(text.iter().map(|&b| if b >= 0x80 { b'x' } else { b }));
                    out.extend_from_slice(eol);
                }
                _ => out.extend_from_slice(line),
            }
            continue;
        }
        let Some(c) = text.iter().position(|&b| b == b':') else {
            if feature != "header-line-without-colon" {
                out.extend_from_slice(line);
            }
            continue;
        };
        let (name, value) = (&text[..c], &text[c..]);
        let mut name = name.to_vec();
        let mut value = value.to_vec();
        match feature {
            "high-byte-header-name" => {
                for b in &mut name {
                    if *b >= 0x80 {
                        *b = b'x';
                    }
                }
            }
            "space-before-colon" => {
                while name.last().is_some_and(|&b| b == b' ' || b == b'\t') {
                    name.pop();
                }
            }
            "header-name-charset" => {
                for b in &mut name {
                    if *b < 0x80 && !is_tchar(*b) {
                        *b = b'x';
                    }
                }
            }
            "high-byte-header-value" => {
                for b in &mut value {
                    if *b >= 0x80 {
                        *b = b'x';
                    }
                }
            }
            _ => {}
        }
        out.extend_from_slice(&name);
        out.extend_from_slice(&value);
        out.extend_from_slice(eol);
    }
    out
}

/// What explains a disagreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Explanation {
    /// Neutralizing exactly these features (a smallest such set) makes the three agree.
    Features(Vec<&'static str>),
    /// picohttpparser and fuzzdiff still need more bytes where httparse already rejects, and
    /// no feature explains it: httparse commits to a reject on an invalid prefix earlier.
    EarlyReject,
    /// More header lines than the 64 slots both sides were given: the parsers differ in when
    /// they notice (picohttpparser and fuzzdiff reject on seeing a 65th line start,
    /// httparse waits for it to be complete).
    HeaderCap,
    /// Not explained by any known feature. A non-zero count is a class this crate does
    /// not know about.
    Unexplained,
}

fn header_lines(op: Op, data: &[u8]) -> usize {
    let skip = usize::from(op != Op::Headers);
    data.split(|&b| b == b'\n')
        .skip(skip)
        .take_while(|l| !l.is_empty() && *l != b"\r")
        .count()
}

/// Find the smallest set of features whose neutralization makes the three parsers agree.
pub fn explain(op: Op, data: &[u8]) -> Explanation {
    let present = features(op, data);
    for size in 1..=present.len().min(4) {
        let mut idx: Vec<usize> = (0..size).collect();
        loop {
            let mut d = data.to_vec();
            for &i in &idx {
                d = neutralize(op, &d, present[i]);
            }
            if tri_head(op, &d).agrees() {
                return Explanation::Features(idx.iter().map(|&i| present[i]).collect());
            }
            // next combination
            let mut k = size;
            while k > 0 && idx[k - 1] == present.len() - size + k - 1 {
                k -= 1;
            }
            if k == 0 {
                break;
            }
            idx[k - 1] += 1;
            for j in k..size {
                idx[j] = idx[j - 1] + 1;
            }
        }
    }
    let t = tri_head(op, data);
    if t.c == Verdict::Partial && t.rust == Verdict::Partial && t.hp == Verdict::Reject {
        Explanation::EarlyReject
    } else if header_lines(op, data) > 64 {
        Explanation::HeaderCap
    } else {
        Explanation::Unexplained
    }
}

/// Debug rendering of what each parser returned, for the docs and for failure messages:
/// `[picohttpparser, fuzzdiff, httparse]`.
pub fn results_debug(op: Op, data: &[u8]) -> [String; 3] {
    let mut case = Case::new(op, data.to_vec());
    case.max_headers = 64;
    let c = c_head(&case).unwrap_or(Norm::Reject);
    let rust = rust_head(&case, Config::default()).0;
    let hp = hp_head(op, data);
    [format!("{c:?}"), format!("{rust:?}"), format!("{hp:?}")]
}

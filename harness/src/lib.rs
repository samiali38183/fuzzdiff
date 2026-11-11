//! Differential harness for `fuzzdiff`.
//!
//! [`compare`] takes raw fuzzer bytes, turns them into a [`Case`], runs the C
//! picohttpparser (through a small shim) and the Rust port on it, and classifies
//! the result as an [`Outcome`]. Anything classified as [`Outcome::Divergence`]
//! is a bug in one of the two implementations.

pub mod ffi;
pub mod generate;

use fuzzdiff::{ChunkStatus, ChunkedDecoder, Config, Error, Header, Status};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub use ffi::HeadKind;

/// Which entry point a case exercises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `phr_parse_request`
    Request,
    /// `phr_parse_response`
    Response,
    /// `phr_parse_headers`
    Headers,
    /// `phr_decode_chunked`, possibly over several calls
    Chunked,
}

/// One differential test input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// Entry point under test.
    pub op: Op,
    /// The bytes handed to the parser.
    pub data: Vec<u8>,
    /// `last_len` argument (head parsers only).
    pub last_len: usize,
    /// Header slice capacity (head parsers only).
    pub max_headers: usize,
    /// Run the Rust side with `Config { reject_obs_fold: true }`.
    pub strict: bool,
    /// `consume_trailer` (chunked only).
    pub consume_trailer: bool,
    /// Sizes of the pieces fed to the chunked decoder, one call each.
    pub pieces: Vec<usize>,
}

impl Case {
    /// A case with default parameters: `last_len` 0, 32 headers, whole input in one piece.
    pub fn new(op: Op, data: impl Into<Vec<u8>>) -> Case {
        let data = data.into();
        let pieces = vec![data.len()];
        Case {
            op,
            data,
            last_len: 0,
            max_headers: 32,
            strict: false,
            consume_trailer: false,
            pieces,
        }
    }

    /// Split the input into pieces of `size` bytes (the last may be shorter).
    pub fn with_piece_size(mut self, size: usize) -> Case {
        self.pieces = split_pieces(self.data.len(), size);
        self
    }

    /// Decode fuzzer bytes. The first four bytes are parameters, the rest is the payload.
    ///
    /// * byte 0: bits 0-1 operation, bit 2 strict, bit 3 consume_trailer
    /// * byte 1: `max_headers` is this value modulo 72
    /// * byte 2: bit 0 set means a non-zero `last_len` of `len * (byte >> 1) / 127`
    /// * byte 3: chunk piece size (0 means one piece)
    pub fn from_bytes(bytes: &[u8]) -> Case {
        let param = |i: usize| bytes.get(i).copied().unwrap_or(0);
        let payload = bytes.get(4..).unwrap_or_default().to_vec();
        let op = match param(0) & 3 {
            0 => Op::Request,
            1 => Op::Response,
            2 => Op::Headers,
            _ => Op::Chunked,
        };
        let last_len = if param(2) & 1 == 1 {
            payload.len() * usize::from(param(2) >> 1) / 127
        } else {
            0
        };
        let pieces = split_pieces(payload.len(), usize::from(param(3)));
        Case {
            op,
            last_len,
            max_headers: usize::from(param(1)) % 72,
            strict: param(0) & 4 != 0,
            consume_trailer: param(0) & 8 != 0,
            pieces,
            data: payload,
        }
    }

    /// Encode as fuzzer bytes (inverse of [`Case::from_bytes`] for the cases the
    /// seed corpus needs: `last_len` of 0 or the full length, uniform pieces).
    pub fn to_seed_bytes(&self) -> Vec<u8> {
        let op = match self.op {
            Op::Request => 0,
            Op::Response => 1,
            Op::Headers => 2,
            Op::Chunked => 3,
        };
        let b0 = op | (u8::from(self.strict) << 2) | (u8::from(self.consume_trailer) << 3);
        let b1 = u8::try_from(self.max_headers.min(71)).unwrap_or(71);
        let b2 = if self.last_len == 0 { 0 } else { 255 };
        let b3 = match self.pieces.as_slice() {
            [only] if *only == self.data.len() => 0,
            [first, ..] => u8::try_from(*first).unwrap_or(255),
            [] => 0,
        };
        let mut v = vec![b0, b1, b2, b3];
        v.extend_from_slice(&self.data);
        v
    }
}

fn split_pieces(len: usize, size: usize) -> Vec<usize> {
    if size == 0 || len == 0 {
        return vec![len];
    }
    let mut v = vec![size; len / size];
    if len > (len / size) * size {
        v.push(len % size);
    }
    v
}

/// A documented place where the Rust port is allowed to differ from C.
/// Every variant is listed in `docs/DEVIATIONS.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deviation {
    /// D1: opt-in `reject_obs_fold` rejects folded header lines.
    ObsFold,
    /// D2: `last_len > len` is an error in Rust; the C library reads out of bounds.
    LastLenOutOfRange,
}

impl Deviation {
    /// The identifier used in `docs/DEVIATIONS.md`.
    pub fn id(self) -> &'static str {
        match self {
            Deviation::ObsFold => "D1",
            Deviation::LastLenOutOfRange => "D2",
        }
    }

    /// All deviations.
    pub const ALL: [Deviation; 2] = [Deviation::ObsFold, Deviation::LastLenOutOfRange];
}

/// Why a case was flagged as a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivergenceKind {
    /// The Rust parser panicked.
    RustPanic,
    /// The two implementations returned different results.
    ResultMismatch,
    /// C returned an offset/length outside the input (an out-of-bounds pointer).
    CBadOffsets,
}

/// A bug found by the harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    /// Category.
    pub kind: DivergenceKind,
    /// Human-readable description with both results.
    pub detail: String,
}

/// Result of running one case on both implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Same result: same offsets, fields and bytes, or both "partial".
    Equal,
    /// Both rejected the input.
    BothReject,
    /// Rust rejects (or stops early) in a documented, intentional way.
    RustStricter(Deviation),
    /// A bug. Never expected.
    Divergence(Divergence),
}

impl Outcome {
    /// `true` if this is a [`Outcome::Divergence`].
    pub fn is_divergence(&self) -> bool {
        matches!(self, Outcome::Divergence(_))
    }
}

/// Run fuzzer bytes through both implementations.
pub fn compare(bytes: &[u8]) -> Outcome {
    compare_case(&Case::from_bytes(bytes))
}

/// Run a [`Case`] through both implementations.
pub fn compare_case(case: &Case) -> Outcome {
    match case.op {
        Op::Chunked => compare_chunked(case),
        _ => compare_head(case),
    }
}

// ---------------------------------------------------------------------------
// Head parsers

/// Result of a head parse in a form both sides can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Norm {
    Partial,
    Reject,
    Other(i64),
    Done(Box<Head>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Head {
    consumed: usize,
    method: Vec<u8>,
    path: Vec<u8>,
    reason: Vec<u8>,
    minor_version: i32,
    status: i32,
    headers: Vec<(Option<Vec<u8>>, Vec<u8>)>,
}

fn head_kind(op: Op) -> HeadKind {
    match op {
        Op::Request => HeadKind::Request,
        Op::Response => HeadKind::Response,
        _ => HeadKind::Headers,
    }
}

fn compare_head(case: &Case) -> Outcome {
    let cfg = Config {
        reject_obs_fold: case.strict,
    };
    let rust = match catch_unwind(AssertUnwindSafe(|| rust_head(case, cfg))) {
        Ok(r) => r,
        Err(p) => {
            return diverge(
                DivergenceKind::RustPanic,
                format!("Rust panicked: {}", panic_text(&p)),
            );
        }
    };

    if case.last_len > case.data.len() {
        // The C library reads out of bounds here, so it is not called.
        return match rust.1 {
            Some(Error::InvalidLastLen) => Outcome::RustStricter(Deviation::LastLenOutOfRange),
            other => diverge(
                DivergenceKind::ResultMismatch,
                format!(
                    "last_len > len: expected InvalidLastLen, Rust gave {:?} / {other:?}",
                    rust.0
                ),
            ),
        };
    }

    let c = c_head(case);
    let c = match c {
        Ok(c) => c,
        Err(detail) => return diverge(DivergenceKind::CBadOffsets, detail),
    };

    if cfg.reject_obs_fold && rust.1 == Some(Error::ObsFold) {
        // Only a folded line (a line feed followed by a space or tab) justifies this error.
        // Without that check, a bug that reports ObsFold on a normal header would look like
        // the documented deviation and pass.
        return if c == Norm::Reject {
            Outcome::BothReject
        } else if has_fold(&case.data) {
            Outcome::RustStricter(Deviation::ObsFold)
        } else {
            diverge(
                DivergenceKind::ResultMismatch,
                "ObsFold reported but the input has no folded line".to_string(),
            )
        };
    }
    if c == rust.0 {
        return if c == Norm::Reject {
            Outcome::BothReject
        } else {
            Outcome::Equal
        };
    }
    diverge(
        DivergenceKind::ResultMismatch,
        format!("C: {c:?}\nRust: {:?} ({:?})", rust.0, rust.1),
    )
}

fn rust_head(case: &Case, cfg: Config) -> (Norm, Option<Error>) {
    let mut headers = vec![Header::EMPTY; case.max_headers];
    let data = &case.data[..];
    let to_headers = |n: usize, h: &[Header<'_>]| -> Vec<(Option<Vec<u8>>, Vec<u8>)> {
        h.iter()
            .take(n)
            .map(|h| (h.name.map(<[u8]>::to_vec), h.value.to_vec()))
            .collect()
    };
    let result = match case.op {
        Op::Request => cfg
            .parse_request(data, &mut headers, case.last_len)
            .map(|s| match s {
                Status::Partial => Norm::Partial,
                Status::Complete(r) => Norm::Done(Box::new(Head {
                    consumed: r.consumed,
                    method: r.method.to_vec(),
                    path: r.path.to_vec(),
                    minor_version: i32::from(r.minor_version),
                    headers: to_headers(r.num_headers, &headers),
                    ..Head::default()
                })),
            }),
        Op::Response => cfg
            .parse_response(data, &mut headers, case.last_len)
            .map(|s| match s {
                Status::Partial => Norm::Partial,
                Status::Complete(r) => Norm::Done(Box::new(Head {
                    consumed: r.consumed,
                    reason: r.reason.to_vec(),
                    minor_version: i32::from(r.minor_version),
                    status: i32::from(r.status),
                    headers: to_headers(r.num_headers, &headers),
                    ..Head::default()
                })),
            }),
        _ => cfg
            .parse_headers(data, &mut headers, case.last_len)
            .map(|s| match s {
                Status::Partial => Norm::Partial,
                Status::Complete(r) => Norm::Done(Box::new(Head {
                    consumed: r.consumed,
                    headers: to_headers(r.num_headers, &headers),
                    ..Head::default()
                })),
            }),
    };
    match result {
        Ok(n) => (n, None),
        Err(e) => (Norm::Reject, Some(e)),
    }
}

fn c_head(case: &Case) -> Result<Norm, String> {
    let c = ffi::c_parse(
        head_kind(case.op),
        &case.data,
        case.last_len,
        case.max_headers,
    );
    match c.ret {
        -2 => return Ok(Norm::Partial),
        -1 => return Ok(Norm::Reject),
        r if r < 0 => return Ok(Norm::Other(r)),
        _ => {}
    }
    let bad = |what: &str| {
        format!(
            "C returned out-of-range {what} for input of {} bytes: {c:?}",
            case.data.len()
        )
    };
    let slice = |(off, len): (usize, usize)| -> Option<Vec<u8>> {
        let end = off.checked_add(len)?;
        case.data.get(off..end).map(<[u8]>::to_vec)
    };
    let consumed = usize::try_from(c.ret).map_err(|_| bad("return value"))?;
    if consumed > case.data.len() {
        return Err(bad("consumed length"));
    }
    let mut headers = Vec::with_capacity(c.headers.len());
    for h in &c.headers {
        let name = match h.name {
            None => None,
            Some(span) => Some(slice(span).ok_or_else(|| bad("header name"))?),
        };
        headers.push((name, slice(h.value).ok_or_else(|| bad("header value"))?));
    }
    Ok(Norm::Done(Box::new(Head {
        consumed,
        method: slice(c.method).ok_or_else(|| bad("method"))?,
        path: slice(c.path).ok_or_else(|| bad("path"))?,
        reason: slice(c.reason).ok_or_else(|| bad("reason"))?,
        minor_version: c.minor_version,
        status: c.status,
        headers,
    })))
}

// ---------------------------------------------------------------------------
// Chunked decoder

fn rust_chunked(case: &Case) -> ffi::CChunked {
    let mut dec = ChunkedDecoder::new(case.consume_trailer);
    let mut res = ffi::CChunked {
        steps: Vec::new(),
        out: Vec::new(),
        left: Vec::new(),
    };
    let mut off: usize = 0;
    for &n in &case.pieces {
        let end = off.saturating_add(n).min(case.data.len());
        let mut buf = case.data.get(off..end).unwrap_or_default().to_vec();
        off = end;
        let d = dec.decode(&mut buf);
        let ret = match d.status {
            ChunkStatus::Incomplete => -2,
            ChunkStatus::Error(_) => -1,
            ChunkStatus::Complete { leftover } => i64::try_from(leftover).unwrap_or(i64::MAX),
        };
        res.steps.push(ffi::CChunkStep {
            ret,
            newlen: d.len,
            in_data: dec.is_in_data(),
        });
        res.out
            .extend_from_slice(buf.get(..d.len).unwrap_or_default());
        if let ChunkStatus::Complete { leftover } = d.status {
            res.left
                .extend_from_slice(buf.get(d.len..d.len + leftover).unwrap_or_default());
        }
        if ret != -2 {
            break;
        }
    }
    res
}

fn compare_chunked(case: &Case) -> Outcome {
    let rust = match catch_unwind(AssertUnwindSafe(|| rust_chunked(case))) {
        Ok(r) => r,
        Err(p) => {
            return diverge(
                DivergenceKind::RustPanic,
                format!("Rust panicked: {}", panic_text(&p)),
            );
        }
    };
    let c = ffi::c_chunked(&case.data, &case.pieces, case.consume_trailer);
    if c != rust {
        return diverge(
            DivergenceKind::ResultMismatch,
            format!("C: {c:?}\nRust: {rust:?}"),
        );
    }
    match c.steps.last() {
        Some(s) if s.ret == -1 => Outcome::BothReject,
        _ => Outcome::Equal,
    }
}

// ---------------------------------------------------------------------------

fn diverge(kind: DivergenceKind, detail: String) -> Outcome {
    Outcome::Divergence(Divergence { kind, detail })
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic".to_string()
    }
}

/// Render a case so it can be pasted into a bug report.
pub fn describe(case: &Case) -> String {
    format!(
        "{:?} last_len={} max_headers={} strict={} trailer={} pieces={:?} data={:?}",
        case.op,
        case.last_len,
        case.max_headers,
        case.strict,
        case.consume_trailer,
        case.pieces,
        String::from_utf8_lossy(&case.data)
    )
}

/// True when the bytes contain a line feed followed by a space or tab, which is the
/// necessary condition for an obsolete folded header line.
fn has_fold(data: &[u8]) -> bool {
    data.windows(2)
        .any(|w| w[0] == b'\n' && (w[1] == b' ' || w[1] == b'\t'))
}

#[cfg(test)]
mod fold_tests {
    use super::has_fold;

    #[test]
    fn fold_needs_a_line_feed_then_space_or_tab() {
        assert!(has_fold(b"GET / HTTP/1.1\r\nA: b\r\n c\r\n\r\n"));
        assert!(has_fold(b"A: b\n\tc\n"));
        assert!(!has_fold(
            b"GET / HTTP/1.1\r\nX-Forwarded-For: 1.2.3.4\r\n\r\n"
        ));
        assert!(!has_fold(b"a b\tc"));
        assert!(!has_fold(b"\n"));
    }
}

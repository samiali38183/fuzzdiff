//! Curated minimal inputs for the three-way comparison with `httparse`.
//!
//! Each [`Example`] is one input with the outcome pattern it must produce
//! (see [`crate::httparse_oracle::Triple::pattern`]). `harness/tests/httparse_classes.rs`
//! fails if any pattern changes, so a change in picohttpparser (vendored at a fixed
//! commit), in fuzzdiff, or in httparse (pinned, so bumping it is a deliberate act) that
//! creates a new disagreement, or removes an old one, is noticed.
//!
//! The pattern is three letters in the order picohttpparser, fuzzdiff, httparse:
//! `C` complete, `P` partial, `R` reject. A trailing `!` means all three completed but
//! returned different fields.

use crate::httparse_oracle::{tri_chunk_size, tri_head, Triple};
use crate::Op;

/// What kind of input an [`Example`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A request head.
    Request,
    /// A response head.
    Response,
    /// A bare header block.
    Headers,
    /// A whole chunked body whose first chunk-size line is the thing under test.
    ChunkSize,
}

/// One curated input.
#[derive(Debug, Clone, Copy)]
pub struct Example {
    /// Class id, a key of [`CLASSES`].
    pub class: &'static str,
    /// Short label shown in the generated table.
    pub label: &'static str,
    /// Which parser entry point.
    pub kind: Kind,
    /// The bytes.
    pub bytes: &'static [u8],
    /// Expected pattern (picohttpparser, fuzzdiff, httparse).
    pub expect: &'static str,
}

impl Example {
    /// Run the example through the three parsers.
    pub fn run(&self) -> Triple {
        match self.kind {
            Kind::Request => tri_head(Op::Request, self.bytes),
            Kind::Response => tri_head(Op::Response, self.bytes),
            Kind::Headers => tri_head(Op::Headers, self.bytes),
            Kind::ChunkSize => tri_chunk_size(self.bytes),
        }
    }
}

/// Class id, title, and the RFC text that governs it.
pub const CLASSES: &[(&str, &str, &str)] = &[
    (
        "agree",
        "Accepted by all three (context)",
        "RFC 9112 6.1, 6.2, 6.3",
    ),
    ("fold", "Obsolete line folding", "RFC 9112 5.2"),
    (
        "ws-first",
        "Whitespace before the first header line",
        "RFC 9112 2.2",
    ),
    (
        "sp-count",
        "Number of SP between start-line elements",
        "RFC 9112 3, 4",
    ),
    (
        "sep",
        "Other whitespace as a start-line separator",
        "RFC 9112 3",
    ),
    ("version", "HTTP version digits", "RFC 9112 2.3"),
    (
        "hi-target",
        "Octets 0x80-0xFF in the request-target",
        "RFC 9112 3.2, RFC 9110 4.1",
    ),
    (
        "hi-value",
        "Octets 0x80-0xFF in a field value",
        "RFC 9110 5.5",
    ),
    (
        "hi-name",
        "Octets 0x80-0xFF in a field name",
        "RFC 9110 5.1, 5.6.2",
    ),
    (
        "name-charset",
        "Field-name characters outside tchar",
        "RFC 9110 5.1, 5.6.2",
    ),
    (
        "colon-ws",
        "Whitespace between field name and colon",
        "RFC 9112 5.1",
    ),
    ("bare-lf", "Bare LF as line terminator", "RFC 9112 2.2"),
    ("bare-cr", "Bare CR", "RFC 9112 2.2"),
    (
        "ctl",
        "NUL and other control octets",
        "RFC 9110 5.5, RFC 9112 3.2",
    ),
    (
        "lead-crlf",
        "Empty lines before the start-line",
        "RFC 9112 2.2",
    ),
    ("status", "Status-code range", "RFC 9112 4"),
    ("reason", "Reason-phrase content", "RFC 9112 4"),
    (
        "timing",
        "Partial input: when a parser gives up",
        "none (implementation behaviour)",
    ),
    ("chunk-ext", "Chunk extensions", "RFC 9112 7.1.1"),
    ("chunk-size", "Chunk-size line shape", "RFC 9112 7.1"),
];

macro_rules! ex {
    ($class:literal, $label:literal, $kind:ident, $bytes:literal, $expect:literal) => {
        Example {
            class: $class,
            label: $label,
            kind: Kind::$kind,
            bytes: $bytes,
            expect: $expect,
        }
    };
}

/// The curated examples.
#[rustfmt::skip]
pub const EXAMPLES: &[Example] = &[
    ex!("agree", "baseline request", Request, b"GET / HTTP/1.1\r\nHost: a\r\n\r\n", "CCC"),
    ex!("agree", "Content-Length and Transfer-Encoding both present", Request, b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n", "CCC"),
    ex!("agree", "duplicate Content-Length, different values", Request, b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 4\r\nContent-Length: 5\r\n\r\n", "CCC"),
    ex!("agree", "Transfer-Encoding: xchunked", Request, b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: xchunked\r\n\r\n", "CCC"),
    ex!("agree", "absolute-form target", Request, b"GET http://a/ HTTP/1.1\r\nHost: a\r\n\r\n", "CCC"),
    ex!("agree", "asterisk-form target", Request, b"OPTIONS * HTTP/1.1\r\nHost: a\r\n\r\n", "CCC"),
    ex!("agree", "lower-case method", Request, b"get / HTTP/1.1\r\n\r\n", "CCC"),
    ex!("agree", "non-token method", Request, b"G@T / HTTP/1.1\r\n\r\n", "RRR"),
    ex!("agree", "trailing SP after the version", Request, b"GET / HTTP/1.1 \r\n\r\n", "RRR"),
    ex!("agree", "HTAB inside a field value", Request, b"GET / HTTP/1.1\r\nA: b\tc\r\n\r\n", "CCC"),
    ex!("agree", "empty field name", Request, b"GET / HTTP/1.1\r\n: x\r\n\r\n", "RRR"),
    ex!("agree", "header line without a colon", Request, b"GET / HTTP/1.1\r\nfoo\r\n\r\n", "RRR"),
    ex!("agree", "response with no reason phrase", Response, b"HTTP/1.1 200\r\n\r\n", "CCC"),

    ex!("fold", "request, SP continuation", Request, b"GET / HTTP/1.1\r\nA: b\r\n c\r\n\r\n", "CCR"),
    ex!("fold", "request, HTAB continuation", Request, b"GET / HTTP/1.1\r\nA: b\r\n\tc\r\n\r\n", "CCR"),
    ex!("fold", "header block, SP continuation", Headers, b"A: b\r\n c\r\n\r\n", "CCR"),
    ex!("fold", "response, SP continuation", Response, b"HTTP/1.1 200 OK\r\nA: b\r\n c\r\n\r\n", "CCR"),
    ex!("ws-first", "request, SP before the first header", Request, b"GET / HTTP/1.1\r\n A: b\r\n\r\n", "RRR"),
    ex!("ws-first", "header block, SP before the first header", Headers, b" A: b\r\n\r\n", "RRR"),

    ex!("sp-count", "two SP after the method", Request, b"GET  / HTTP/1.1\r\n\r\n", "CCR"),
    ex!("sp-count", "two SP before the version", Request, b"GET /  HTTP/1.1\r\n\r\n", "CCR"),
    ex!("sp-count", "two SP after the status code", Response, b"HTTP/1.1 200  OK\r\n\r\n", "CCC!"),
    ex!("sep", "HTAB after the method", Request, b"GET\t/ HTTP/1.1\r\n\r\n", "RRR"),
    ex!("sep", "HTAB before the version", Request, b"GET /\tHTTP/1.1\r\n\r\n", "RRR"),

    ex!("version", "HTTP/1.2 request", Request, b"GET / HTTP/1.2\r\n\r\n", "CCR"),
    ex!("version", "HTTP/1.9 request", Request, b"GET / HTTP/1.9\r\n\r\n", "CCR"),
    ex!("version", "HTTP/1.2 response", Response, b"HTTP/1.2 200 OK\r\n\r\n", "CCR"),
    ex!("version", "HTTP/2.0 request", Request, b"GET / HTTP/2.0\r\n\r\n", "RRR"),
    ex!("version", "HTTP/0.9 request", Request, b"GET / HTTP/0.9\r\n\r\n", "RRR"),
    ex!("version", "HTTP/1.10 request", Request, b"GET / HTTP/1.10\r\n\r\n", "RRR"),
    ex!("version", "lower-case http/1.1", Request, b"GET / http/1.1\r\n\r\n", "RRR"),

    ex!("hi-target", "0x80 in the target", Request, b"GET /\x80 HTTP/1.1\r\n\r\n", "CCR"),
    ex!("hi-target", "0xFF in the target", Request, b"GET /\xff HTTP/1.1\r\n\r\n", "CCR"),
    ex!("hi-value", "0x80 in a field value", Request, b"GET / HTTP/1.1\r\nA: \x80\r\n\r\n", "CCC"),
    ex!("hi-value", "0xFF in a response field value", Response, b"HTTP/1.1 200 OK\r\nA: \xff\r\n\r\n", "CCC"),
    ex!("hi-name", "0x80 in a field name", Request, b"GET / HTTP/1.1\r\n\x80: x\r\n\r\n", "RRR"),
    ex!("name-charset", "@ in a field name", Request, b"GET / HTTP/1.1\r\nA@B: x\r\n\r\n", "RRR"),
    ex!("name-charset", "( in a field name", Request, b"GET / HTTP/1.1\r\nA(B: x\r\n\r\n", "RRR"),
    ex!("name-charset", "SP inside a field name", Request, b"GET / HTTP/1.1\r\nA B: x\r\n\r\n", "RRR"),
    ex!("name-charset", "underscore and tilde in a field name (valid tchar)", Request, b"GET / HTTP/1.1\r\nA_B~: x\r\n\r\n", "CCC"),
    ex!("colon-ws", "SP before the colon, request", Request, b"GET / HTTP/1.1\r\nA : b\r\n\r\n", "RRR"),
    ex!("colon-ws", "SP before the colon, response", Response, b"HTTP/1.1 200 OK\r\nA : b\r\n\r\n", "RRR"),
    ex!("colon-ws", "HTAB before the colon", Request, b"GET / HTTP/1.1\r\nA\t: b\r\n\r\n", "RRR"),

    ex!("bare-lf", "LF-only line endings", Request, b"GET / HTTP/1.1\nHost: a\n\n", "CCC"),
    ex!("bare-lf", "LF after the start-line only", Request, b"GET / HTTP/1.1\nHost: a\r\n\r\n", "CCC"),
    ex!("bare-cr", "bare CR after the start-line", Request, b"GET / HTTP/1.1\r\r\nHost: a\r\n\r\n", "RRR"),
    ex!("bare-cr", "bare CR inside a field value", Request, b"GET / HTTP/1.1\r\nA: b\rc\r\n\r\n", "RRR"),
    ex!("ctl", "NUL in a field value", Request, b"GET / HTTP/1.1\r\nA: b\x00c\r\n\r\n", "RRR"),
    ex!("ctl", "NUL in the target", Request, b"GET /\x00 HTTP/1.1\r\n\r\n", "RRR"),
    ex!("ctl", "DEL in a field value", Request, b"GET / HTTP/1.1\r\nA: b\x7fc\r\n\r\n", "RRR"),
    ex!("ctl", "0x01 in a field value", Request, b"GET / HTTP/1.1\r\nA: b\x01c\r\n\r\n", "RRR"),
    ex!("ctl", "0x01 in the target", Request, b"GET /\x01 HTTP/1.1\r\n\r\n", "RRR"),
    ex!("lead-crlf", "one empty line before the request", Request, b"\r\nGET / HTTP/1.1\r\n\r\n", "CCC"),
    ex!("lead-crlf", "two empty lines before the request", Request, b"\r\n\r\nGET / HTTP/1.1\r\n\r\n", "RRC"),

    ex!("status", "status 600", Response, b"HTTP/1.1 600 X\r\n\r\n", "RRC"),
    ex!("status", "status 099", Response, b"HTTP/1.1 099 X\r\n\r\n", "RRC"),
    ex!("status", "status 999", Response, b"HTTP/1.1 999 X\r\n\r\n", "RRC"),
    ex!("status", "two-digit status", Response, b"HTTP/1.1 20 X\r\n\r\n", "RRR"),
    ex!("reason", "0xFF in the reason phrase", Response, b"HTTP/1.1 200 OK\xff\r\n\r\n", "CCC!"),
    ex!("reason", "HTAB in the reason phrase", Response, b"HTTP/1.1 200 \tOK\r\n\r\n", "CCC"),
    ex!("reason", "0x01 in the reason phrase", Response, b"HTTP/1.1 200 O\x01K\r\n\r\n", "RRR"),

    ex!("timing", "version cut after 'HTTP/1.'", Request, b"GET / HTTP/1.", "PPP"),
    ex!("timing", "invalid version prefix, 8 bytes available", Request, b"GET / HX\r\n\r\nxy", "PPR"),
    ex!("timing", "invalid method byte after a full method", Request, b"GE\x01", "RRR"),
    ex!("timing", "obs-fold line cut before its end", Request, b"GET / HTTP/1.1\r\nA: b\r\n c", "PPR"),

    ex!("chunk-ext", "no extension (baseline)", ChunkSize, b"5\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-ext", "name=value extension", ChunkSize, b"5;a=b\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-ext", "name-only extension", ChunkSize, b"5;a\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-ext", "empty extension (';' then CRLF)", ChunkSize, b"5;\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-ext", "quoted-string value", ChunkSize, b"5;a=\"b c\"\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-ext", "BWS around ';' and '='", ChunkSize, b"5 ; a = b\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-ext", "0xFF in an extension", ChunkSize, b"5;a=\xff\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-ext", "NUL in an extension", ChunkSize, b"5;a=\x00\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-ext", "bare CR in an extension", ChunkSize, b"5;a=b\rc\r\nhello\r\n0\r\n\r\n", "RRR"),
    ex!("chunk-ext", "bare LF inside an extension", ChunkSize, b"5;a=b\nhello\r\n0\r\n\r\n", "RRC"),
    ex!("chunk-size", "SP after the size", ChunkSize, b"5 \r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-size", "HTAB after the size", ChunkSize, b"5\t\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-size", "bare LF after the size", ChunkSize, b"5\nhello\r\n0\r\n\r\n", "RRR"),
    ex!("chunk-size", "upper-case hex digit", ChunkSize, b"A\r\n0123456789\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-size", "leading zeros", ChunkSize, b"0005\r\nhello\r\n0\r\n\r\n", "CCC"),
    ex!("chunk-size", "0x prefix", ChunkSize, b"0x5\r\nhello\r\n0\r\n\r\n", "RRR"),
    ex!("chunk-size", "plus sign", ChunkSize, b"+5\r\nhello\r\n0\r\n\r\n", "RRR"),
    ex!("chunk-size", "empty size", ChunkSize, b"\r\nhello\r\n0\r\n\r\n", "RRC"),
    ex!("chunk-size", "non-hex byte after the size", ChunkSize, b"5g\r\nhello\r\n0\r\n\r\n", "RRR"),
    ex!("chunk-size", "17 hex digits (overflow of u64)", ChunkSize, b"10000000000000000\r\nhello", "RRR"),
    ex!("chunk-size", "18 hex digits with leading zeros", ChunkSize, b"000000000000000005\r\nhello\r\n0\r\n\r\n", "RRR"),
];

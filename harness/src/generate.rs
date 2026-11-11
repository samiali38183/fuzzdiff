//! Deterministic input generators: well-formed HTTP messages and mutations of them.
//!
//! Used by the proptest suite and by the mutant-detection test, which needs a
//! reproducible stream of inputs so its "inputs until detection" numbers mean something.

use crate::{Case, Op};

/// xorshift64* generator. Not for cryptography.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// Seeded generator (a zero seed is remapped).
    pub fn new(seed: u64) -> Rng {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-ish value in `0..n` (`n` must be non-zero).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// `true` with probability `num/den`.
    pub fn chance(&mut self, num: usize, den: usize) -> bool {
        self.below(den) < num
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const METHODS: &[&str] = &[
    "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "M-SEARCH", "X.Y",
];
const NAMES: &[&str] = &[
    "Host",
    "User-Agent",
    "Accept",
    "Content-Length",
    "Transfer-Encoding",
    "Cookie",
    "X-Forwarded-For",
    "Connection",
    "a",
    "X",
    "Set-Cookie",
    "ETag",
];
const WORDS: &[&str] = &[
    "alpha", "beta", "gamma", "/", "?", "=", "&", " ", "\t", ";", "q=0.9", "\u{e9}", "%00", "::",
];

fn token(r: &mut Rng) -> String {
    if r.chance(3, 4) {
        return (*r.pick(NAMES)).to_string();
    }
    let n = 1 + r.below(24);
    (0..n)
        .map(|_| *r.pick(b"abcXYZ019-_.!#$%&'*+^`|~") as char)
        .collect()
}

fn value(r: &mut Rng) -> Vec<u8> {
    let mut v = Vec::new();
    if r.chance(1, 5) {
        v.extend_from_slice(b" \t");
    }
    for _ in 0..r.below(8) {
        v.extend_from_slice(r.pick(WORDS).as_bytes());
    }
    if r.chance(1, 6) {
        let n = 10 + r.below(50);
        v.extend((0..n).map(|_| b'a' + (r.below(26) as u8)));
    }
    if r.chance(1, 5) {
        v.extend_from_slice(b"  \t ");
    }
    v
}

fn eol(r: &mut Rng, lf_only: bool) -> &'static [u8] {
    if lf_only && r.chance(1, 2) {
        b"\n"
    } else {
        b"\r\n"
    }
}

fn header_block(r: &mut Rng, lf_only: bool) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..r.below(12) {
        v.extend_from_slice(token(r).as_bytes());
        v.push(b':');
        if r.chance(4, 5) {
            v.push(b' ');
        }
        v.extend(value(r));
        v.extend_from_slice(eol(r, lf_only));
        if r.chance(1, 12) {
            v.extend_from_slice(if r.chance(1, 2) { b" " } else { b"\t" });
            v.extend(value(r));
            v.extend_from_slice(eol(r, lf_only));
        }
    }
    v.extend_from_slice(eol(r, lf_only));
    v
}

/// A well-formed header block ending in an empty line (for `parse_headers`).
pub fn valid_headers(r: &mut Rng) -> Vec<u8> {
    let lf = r.chance(1, 8);
    header_block(r, lf)
}

/// A well-formed request head, sometimes followed by a body.
pub fn valid_request(r: &mut Rng) -> Vec<u8> {
    let lf = r.chance(1, 8);
    let mut v = Vec::new();
    if r.chance(1, 10) {
        v.extend_from_slice(b"\r\n");
    }
    v.extend_from_slice(r.pick(METHODS).as_bytes());
    v.extend(std::iter::repeat_n(
        b' ',
        1 + usize::from(r.chance(1, 8)) * r.below(3),
    ));
    v.push(b'/');
    for _ in 0..r.below(6) {
        v.extend_from_slice(
            r.pick(WORDS)
                .as_bytes()
                .iter()
                .filter(|&&b| b != b' ' && b != b'\t')
                .copied()
                .collect::<Vec<_>>()
                .as_slice(),
        );
    }
    v.push(b' ');
    v.extend_from_slice(format!("HTTP/1.{}", r.below(10)).as_bytes());
    v.extend_from_slice(eol(r, lf));
    v.extend(header_block(r, lf));
    if r.chance(1, 4) {
        v.extend_from_slice(b"body-bytes");
    }
    v
}

/// A well-formed response head.
pub fn valid_response(r: &mut Rng) -> Vec<u8> {
    let lf = r.chance(1, 8);
    let mut v = format!("HTTP/1.{} {}", r.below(10), 100 + r.below(500)).into_bytes();
    if r.chance(9, 10) {
        v.extend(std::iter::repeat_n(b' ', 1 + r.below(2)));
        v.extend_from_slice(
            r.pick(&[
                "OK",
                "Not Found",
                "Internal Server Error",
                "Switching Protocols",
            ])
            .as_bytes(),
        );
    }
    v.extend_from_slice(eol(r, lf));
    v.extend(header_block(r, lf));
    v
}

/// A well-formed chunked body with optional extensions and trailers.
pub fn valid_chunked(r: &mut Rng) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..r.below(6) {
        let n = r.below(40);
        let hex = if r.chance(1, 4) {
            format!("{n:X}")
        } else {
            format!("{n:x}")
        };
        v.extend_from_slice(hex.as_bytes());
        if r.chance(1, 5) {
            v.extend_from_slice(b" \t");
        }
        if r.chance(1, 4) {
            v.extend_from_slice(b";ext=val");
        }
        v.extend_from_slice(b"\r\n");
        v.extend((0..n).map(|_| r.next_u64() as u8));
        v.extend_from_slice(b"\r\n");
    }
    v.extend_from_slice(b"0\r\n");
    for _ in 0..r.below(3) {
        v.extend_from_slice(token(r).as_bytes());
        v.extend_from_slice(b": ");
        v.extend(value(r));
        v.extend_from_slice(if r.chance(1, 8) { b"\n" } else { b"\r\n" });
    }
    v.extend_from_slice(if r.chance(1, 8) { b"\n" } else { b"\r\n" });
    if r.chance(1, 5) {
        v.extend_from_slice(b"GET / HTTP/1.1\r\n\r\n");
    }
    v
}

const INTERESTING: &[&[u8]] = &[
    b"\r\n",
    b"\n",
    b"\r",
    b"\0",
    b" ",
    b"\t",
    b":",
    b";",
    b"\x7f",
    b"\xff",
    b"HTTP/1.",
    b"\r\n\r\n",
    b"ffffffffffffffff",
    b"0",
];

/// Apply one to a few random edits: byte flips, inserts, deletes, truncation,
/// splicing in "interesting" tokens, duplicating a range.
pub fn mutate(r: &mut Rng, data: &mut Vec<u8>) {
    for _ in 0..1 + r.below(3) {
        if data.is_empty() {
            data.extend_from_slice(r.pick(INTERESTING));
            continue;
        }
        let at = r.below(data.len());
        match r.below(6) {
            0 => data[at] = r.next_u64() as u8,
            1 => data[at] ^= 1 << r.below(8),
            2 => {
                data.remove(at);
            }
            3 => data.truncate(at),
            4 => {
                let tok = r.pick(INTERESTING);
                data.splice(at..at, tok.iter().copied());
            }
            _ => {
                let end = (at + 1 + r.below(16)).min(data.len());
                let chunk = data[at..end].to_vec();
                data.splice(at..at, chunk);
            }
        }
    }
}

/// A random case: well-formed or mutated input for a random entry point, with
/// random `last_len`, `max_headers`, strictness and chunk splitting.
pub fn random_case(r: &mut Rng) -> Case {
    let op = *r.pick(&[
        Op::Request,
        Op::Request,
        Op::Response,
        Op::Headers,
        Op::Chunked,
        Op::Chunked,
    ]);
    let mut data = match op {
        Op::Request => valid_request(r),
        Op::Response => valid_response(r),
        Op::Headers => header_block(r, true),
        Op::Chunked => valid_chunked(r),
    };
    if r.chance(2, 3) {
        mutate(r, &mut data);
    }
    let mut c = Case::new(op, data);
    if op == Op::Chunked {
        c.consume_trailer = r.chance(1, 2);
        let size = if r.chance(1, 3) { 0 } else { 1 + r.below(12) };
        c = c.with_piece_size(size);
    } else {
        c.max_headers = *r.pick(&[0, 1, 2, 4, 8, 16, 32, 64]);
        c.strict = r.chance(1, 4);
        if r.chance(1, 3) {
            c.last_len = r.below(c.data.len() + 1);
        }
    }
    c
}

/// Hand-built cases aimed at guards the random generator cannot reach: a chunked body whose
/// framing is 76.9% of the input and over 100 KiB (between the 75% and 80% thresholds of the
/// overhead guard), its 70% and 90% neighbours, and response heads cut off with exactly three
/// bytes after the status-line spaces.
pub fn targeted_cases() -> Vec<Case> {
    fn chunks(payload: usize, count: usize) -> Case {
        // 16 hex digits with leading zeros: 18 bytes of size line, `payload` bytes, CRLF.
        let mut v = Vec::new();
        for _ in 0..count {
            v.extend_from_slice(format!("{payload:016x}\r\n").as_bytes());
            v.extend(std::iter::repeat_n(b'x', payload));
            v.extend_from_slice(b"\r\n");
        }
        Case::new(Op::Chunked, v)
    }
    let mut out = vec![
        // overhead 20/26 = 76.9%: guard fires at /4 (above 75%), not at /5 (above 80%)
        chunks(6, 6000),
        // overhead 20/28 = 71.4%: below both thresholds, must stay incomplete
        chunks(8, 6000),
        // overhead 20/22 = 90.9%: above both
        chunks(2, 6000),
    ];
    for head in [
        &b"HTTP/1.1 2x1"[..],
        b"HTTP/1.1 20x",
        b"HTTP/1.1 x00",
        b"HTTP/1.1  2x1",
        b"HTTP/1.1 600",
        b"HTTP/1.1 200",
    ] {
        out.push(Case::new(Op::Response, head));
    }
    out
}

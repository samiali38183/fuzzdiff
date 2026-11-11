//! Request, response and header-block parsing.

use crate::error::Error;
use crate::mutants::{self, Mutant};

/// Outcome of a parse that did not fail: either a result or "need more bytes".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status<T> {
    /// The head is complete.
    Complete(T),
    /// The buffer ends before the head does. Call again with more bytes,
    /// passing the previous length as `last_len`.
    Partial,
}

impl<T> Status<T> {
    /// `true` for [`Status::Complete`].
    pub fn is_complete(&self) -> bool {
        matches!(self, Status::Complete(_))
    }
}

/// One header line. A folded continuation line has `name == None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Header<'a> {
    /// Header name, or `None` for a continuation line of the previous header.
    pub name: Option<&'a [u8]>,
    /// Header value with trailing spaces and tabs removed.
    pub value: &'a [u8],
}

impl Header<'_> {
    /// Placeholder for initialising a header array: `[Header::EMPTY; 32]`.
    pub const EMPTY: Header<'static> = Header {
        name: None,
        value: &[],
    };
}

/// A parsed request head. The headers are written into the caller's slice;
/// the first `num_headers` entries are valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request<'a> {
    /// Request method, a non-empty token.
    pub method: &'a [u8],
    /// Request target, non-empty, without control characters.
    pub path: &'a [u8],
    /// The `x` of `HTTP/1.x`.
    pub minor_version: u8,
    /// Number of valid entries in the header slice.
    pub num_headers: usize,
    /// Bytes of the buffer used by the head (request line, headers, blank line).
    pub consumed: usize,
}

/// A parsed response head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Response<'a> {
    /// The `x` of `HTTP/1.x`.
    pub minor_version: u8,
    /// Status code, 100..=599.
    pub status: u16,
    /// Reason phrase with leading spaces removed. May be empty.
    pub reason: &'a [u8],
    /// Number of valid entries in the header slice.
    pub num_headers: usize,
    /// Bytes of the buffer used by the head.
    pub consumed: usize,
}

/// A parsed bare header block (for example trailers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Headers {
    /// Number of valid entries in the header slice.
    pub num_headers: usize,
    /// Bytes of the buffer used, including the blank line.
    pub consumed: usize,
}

/// Parser options. The default reproduces the C library exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Config {
    /// Fail with [`Error::ObsFold`] on a folded (obs-fold) header line instead
    /// of returning it as a nameless header. The C library accepts folding.
    pub reject_obs_fold: bool,
}

/// Parse a request head with the default [`Config`].
///
/// `headers.len()` is the maximum number of header lines. `last_len` is the
/// length of the buffer at the previous call (0 on the first), which lets the
/// parser cheaply re-check whether the head is complete.
pub fn parse_request<'a>(
    buf: &'a [u8],
    headers: &mut [Header<'a>],
    last_len: usize,
) -> Result<Status<Request<'a>>, Error> {
    Config::default().parse_request(buf, headers, last_len)
}

/// Parse a response head with the default [`Config`]. See [`parse_request`].
pub fn parse_response<'a>(
    buf: &'a [u8],
    headers: &mut [Header<'a>],
    last_len: usize,
) -> Result<Status<Response<'a>>, Error> {
    Config::default().parse_response(buf, headers, last_len)
}

/// Parse a header block with the default [`Config`]. See [`parse_request`].
pub fn parse_headers<'a>(
    buf: &'a [u8],
    headers: &mut [Header<'a>],
    last_len: usize,
) -> Result<Status<Headers>, Error> {
    Config::default().parse_headers(buf, headers, last_len)
}

impl Config {
    /// Parse a request head. See [`parse_request`].
    pub fn parse_request<'a>(
        &self,
        buf: &'a [u8],
        headers: &mut [Header<'a>],
        last_len: usize,
    ) -> Result<Status<Request<'a>>, Error> {
        finish(self.request(buf, headers, last_len))
    }

    /// Parse a response head. See [`parse_request`].
    pub fn parse_response<'a>(
        &self,
        buf: &'a [u8],
        headers: &mut [Header<'a>],
        last_len: usize,
    ) -> Result<Status<Response<'a>>, Error> {
        finish(self.response(buf, headers, last_len))
    }

    /// Parse a header block. See [`parse_request`].
    pub fn parse_headers<'a>(
        &self,
        buf: &'a [u8],
        headers: &mut [Header<'a>],
        last_len: usize,
    ) -> Result<Status<Headers>, Error> {
        finish(self.header_block(buf, headers, last_len))
    }

    fn request<'a>(
        &self,
        buf: &'a [u8],
        headers: &mut [Header<'a>],
        last_len: usize,
    ) -> Step<Request<'a>> {
        check_complete(buf, last_len)?;
        let mut c = Cursor::new(buf);

        // A client may send a stray CRLF after a previous request body.
        match c.peek()? {
            b'\r' => {
                c.pos += 1;
                c.expect_lf()?;
            }
            b'\n' => c.pos += 1,
            _ => {}
        }

        let method = c.token(b' ')?;
        c.skip_spaces()?;
        let path = c.target()?;
        c.skip_spaces()?;
        if method.is_empty() {
            return Err(Error::EmptyMethod.into());
        }
        if path.is_empty() {
            return Err(Error::EmptyPath.into());
        }
        let minor_version = c.version()?;
        // `version` has already proven that a byte follows the digit; the
        // seeded bug removes that guarantee and indexes directly.
        let next = if mutants::active(Mutant::MissingBoundsCheck) {
            c.buf[c.pos]
        } else {
            c.peek()?
        };
        match next {
            b'\r' => {
                c.pos += 1;
                c.expect_lf()?;
            }
            b'\n' => c.pos += 1,
            _ => return Err(Error::NewLine.into()),
        }

        let num_headers = self.header_lines(&mut c, headers)?;
        Ok(Request {
            method,
            path,
            minor_version,
            num_headers,
            consumed: c.pos,
        })
    }

    fn response<'a>(
        &self,
        buf: &'a [u8],
        headers: &mut [Header<'a>],
        last_len: usize,
    ) -> Step<Response<'a>> {
        check_complete(buf, last_len)?;
        let mut c = Cursor::new(buf);

        let minor_version = c.version()?;
        if c.peek()? != b' ' {
            return Err(Error::Status.into());
        }
        c.skip_spaces()?;
        // Wants at least three digits and one more byte.
        let need = if mutants::active(Mutant::StatusGuard) {
            3
        } else {
            4
        };
        if c.rest().len() < need {
            return Err(Stop::Partial);
        }
        let status = c.status_code()?;

        let line = c.line_value()?;
        let reason = if line.first() == Some(&b' ') {
            let skip = line.iter().position(|&b| b != b' ').unwrap_or(line.len());
            line.get(skip..).unwrap_or_default()
        } else if line.is_empty() {
            line
        } else {
            return Err(Error::StatusGarbage.into());
        };

        let num_headers = self.header_lines(&mut c, headers)?;
        Ok(Response {
            minor_version,
            status,
            reason,
            num_headers,
            consumed: c.pos,
        })
    }

    fn header_block<'a>(
        &self,
        buf: &'a [u8],
        headers: &mut [Header<'a>],
        last_len: usize,
    ) -> Step<Headers> {
        check_complete(buf, last_len)?;
        let mut c = Cursor::new(buf);
        let num_headers = self.header_lines(&mut c, headers)?;
        Ok(Headers {
            num_headers,
            consumed: c.pos,
        })
    }

    /// Header lines up to and including the blank line. Returns the count.
    fn header_lines<'a>(&self, c: &mut Cursor<'a>, out: &mut [Header<'a>]) -> Step<usize> {
        let mut n = 0;
        loop {
            let first = c.peek()?;
            match first {
                b'\r' => {
                    c.pos += 1;
                    c.expect_lf()?;
                    return Ok(n);
                }
                b'\n' => {
                    c.pos += 1;
                    return Ok(n);
                }
                _ => {}
            }
            let Some(slot) = out.get_mut(n) else {
                return Err(Error::TooManyHeaders.into());
            };

            let name = if n != 0 && (first == b' ' || first == b'\t') {
                if self.reject_obs_fold {
                    return Err(Error::ObsFold.into());
                }
                None
            } else {
                // The name ends at the colon with no space allowed before it
                // (see MFSA 2006-33), and the colon must not be the first byte.
                let name = c.token(b':')?;
                if name.is_empty() {
                    return Err(Error::EmptyHeaderName.into());
                }
                c.pos += 1;
                while matches!(c.peek()?, b' ' | b'\t') {
                    c.pos += 1;
                }
                Some(name)
            };

            let raw = c.line_value()?;
            let mut len = raw
                .iter()
                .rposition(|&b| b != b' ' && b != b'\t')
                .map_or(0, |i| i + 1);
            if mutants::active(Mutant::HeaderLenOffByOne) && len > 16 {
                len -= 1;
            }
            *slot = Header {
                name,
                value: raw.get(..len).unwrap_or_default(),
            };
            n += 1;
        }
    }
}

/// Internal early-exit type: "need more bytes" or a hard failure.
pub(crate) enum Stop {
    Partial,
    Fail(Error),
}

impl From<Error> for Stop {
    fn from(e: Error) -> Self {
        Stop::Fail(e)
    }
}

type Step<T> = Result<T, Stop>;

fn finish<T>(r: Step<T>) -> Result<Status<T>, Error> {
    match r {
        Ok(v) => Ok(Status::Complete(v)),
        Err(Stop::Partial) => Ok(Status::Partial),
        Err(Stop::Fail(e)) => Err(e),
    }
}

/// RFC 9110 `tchar`.
const fn is_tchar(b: u8) -> bool {
    matches!(
        b,
        b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_'
            | b'`' | b'|' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z'
    )
}

const fn build_token_table() -> [bool; 256] {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = is_tchar(i as u8);
        i += 1;
    }
    t
}

static TOKEN: [bool; 256] = build_token_table();

/// Control characters (and DEL) that may not appear in a target or value.
/// Bytes of 0x80 and above are allowed, like in the C library.
const fn is_ctl(b: u8) -> bool {
    b < 0x20 || b == 0x7f
}

/// The slowloris shortcut: when `last_len` is non-zero, check that the buffer
/// already holds the blank line that ends the head before parsing anything.
fn check_complete(buf: &[u8], last_len: usize) -> Step<()> {
    if last_len == 0 {
        return Ok(());
    }
    if last_len > buf.len() {
        return Err(Error::InvalidLastLen.into());
    }
    // Rescan the last three bytes of the previous buffer, in case they were
    // the start of a line ending.
    let mut i = last_len.saturating_sub(3);
    let mut line_ends = 0;
    loop {
        match buf.get(i) {
            None => return Err(Stop::Partial),
            Some(b'\r') => {
                i += 1;
                match buf.get(i) {
                    None => return Err(Stop::Partial),
                    Some(b'\n') => line_ends += 1,
                    Some(_) => return Err(Error::NewLine.into()),
                }
            }
            Some(b'\n') => line_ends += 1,
            Some(_) => line_ends = 0,
        }
        i += 1;
        if line_ends == 2 {
            return Ok(());
        }
    }
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Cursor { buf, pos: 0 }
    }

    /// Unread bytes. `pos <= buf.len()` always holds.
    fn rest(&self) -> &'a [u8] {
        self.buf.get(self.pos..).unwrap_or_default()
    }

    /// Current byte, or "need more bytes" at the end of the buffer.
    fn peek(&self) -> Step<u8> {
        self.buf.get(self.pos).copied().ok_or(Stop::Partial)
    }

    fn expect_lf(&mut self) -> Step<()> {
        if self.peek()? != b'\n' {
            return Err(Error::NewLine.into());
        }
        self.pos += 1;
        Ok(())
    }

    /// Step over `pos` (which holds a space) and any further spaces. Needs a
    /// non-space byte to follow.
    fn skip_spaces(&mut self) -> Step<()> {
        loop {
            self.pos += 1;
            if self.peek()? != b' ' {
                return Ok(());
            }
        }
    }

    /// Token characters up to `delim`, which is left unconsumed.
    fn token(&mut self, delim: u8) -> Step<&'a [u8]> {
        let rest = self.rest();
        let end = rest
            .iter()
            .position(|&b| b == delim || !TOKEN[usize::from(b)])
            .ok_or(Stop::Partial)?;
        if rest.get(end) != Some(&delim) {
            return Err(Error::Token.into());
        }
        self.pos += end;
        Ok(rest.get(..end).unwrap_or_default())
    }

    /// Request target: everything up to the next space, no control characters.
    fn target(&mut self) -> Step<&'a [u8]> {
        let rest = self.rest();
        let end = rest
            .iter()
            .position(|&b| b == b' ' || is_ctl(b))
            .ok_or(Stop::Partial)?;
        if rest.get(end) != Some(&b' ') {
            return Err(Error::Control.into());
        }
        self.pos += end;
        Ok(rest.get(..end).unwrap_or_default())
    }

    /// The rest of the line, consuming the line ending (CRLF or bare LF).
    /// Tab is allowed inside; other control characters are not.
    fn line_value(&mut self) -> Step<&'a [u8]> {
        let rest = self.rest();
        let end = rest
            .iter()
            .position(|&b| is_ctl(b) && b != b'\t')
            .ok_or(Stop::Partial)?;
        let value = rest.get(..end).unwrap_or_default();
        match rest.get(end) {
            Some(b'\r') => match rest.get(end + 1) {
                None => return Err(Stop::Partial),
                Some(b'\n') => self.pos += end + 2,
                Some(_) => return Err(Error::NewLine.into()),
            },
            Some(b'\n') => self.pos += end + 1,
            _ => return Err(Error::Control.into()),
        }
        Ok(value)
    }

    /// `HTTP/1.x`. Asks for nine bytes before judging, like the C library, so
    /// the byte after the digit is guaranteed to exist.
    fn version(&mut self) -> Step<u8> {
        let need = if mutants::active(Mutant::MissingBoundsCheck) {
            8
        } else {
            9
        };
        let rest = self.rest();
        if rest.len() < need {
            return Err(Stop::Partial);
        }
        if rest.get(..7) != Some(b"HTTP/1.") {
            return Err(Error::Version.into());
        }
        let Some(&digit @ b'0'..=b'9') = rest.get(7) else {
            return Err(Error::Version.into());
        };
        self.pos += 8;
        Ok(digit - b'0')
    }

    /// Three-digit status code 100..=599.
    fn status_code(&mut self) -> Step<u16> {
        let rest = self.rest();
        let (Some(&h), Some(&t), Some(&o)) = (rest.first(), rest.get(1), rest.get(2)) else {
            return Err(Stop::Partial);
        };
        if !(b'1'..=b'5').contains(&h) || !t.is_ascii_digit() || !o.is_ascii_digit() {
            return Err(Error::Status.into());
        }
        self.pos += 3;
        Ok(u16::from(h - b'0') * 100 + u16::from(t - b'0') * 10 + u16::from(o - b'0'))
    }
}

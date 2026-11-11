//! Incremental decoder for `Transfer-Encoding: chunked` bodies.

use crate::error::Error;
use crate::mutants::{self, Mutant};

/// Framing overhead (bytes) after which the overhead ratio check starts.
const OVERHEAD_LIMIT: u64 = 102_400;

/// Maximum hex digits in a chunk size: the number that fits in a `usize`.
const MAX_HEX_DIGITS: u8 = (usize::BITS / 4) as u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Size,
    Ext,
    HeaderLf,
    Data,
    DataCr,
    DataLf,
    TrailerHead,
    TrailerMiddle,
}

/// Result of one [`ChunkedDecoder::decode`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkStatus {
    /// All input was consumed and the body is not finished. Feed more bytes.
    Incomplete,
    /// The terminating chunk (and trailers, if requested) was found.
    /// `leftover` bytes that came after it sit directly behind the decoded
    /// data in the buffer: `buf[len..len + leftover]`.
    Complete {
        /// Number of undecoded bytes after the end of the body.
        leftover: usize,
    },
    /// The input is not valid chunked encoding. The decoder is finished.
    Error(Error),
}

/// What [`ChunkedDecoder::decode`] did to the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoded {
    /// The first `len` bytes of the buffer now hold decoded body data.
    pub len: usize,
    /// Whether decoding finished.
    pub status: ChunkStatus,
}

/// Streaming chunked-body decoder that works in place.
///
/// Call [`decode`](Self::decode) with each newly received slice. The framing
/// (chunk sizes, extensions, CRLFs) is removed and the body bytes are moved to
/// the front of the slice.
///
/// ```
/// use fuzzdiff::{ChunkStatus, ChunkedDecoder};
///
/// let mut dec = ChunkedDecoder::new(false);
/// let mut buf = *b"6\r\nhello \r\n5\r\nworld\r\n0\r\n";
/// let out = dec.decode(&mut buf);
/// assert_eq!(&buf[..out.len], b"hello world");
/// assert_eq!(out.status, ChunkStatus::Complete { leftover: 0 });
/// ```
#[derive(Debug, Clone)]
pub struct ChunkedDecoder {
    consume_trailer: bool,
    state: State,
    finished: bool,
    bytes_left: usize,
    hex_count: u8,
    total_read: u64,
    total_overhead: u64,
}

impl ChunkedDecoder {
    /// A fresh decoder. With `consume_trailer` the trailer section after the
    /// last chunk is also consumed, otherwise decoding ends right after the
    /// `0` chunk line.
    pub fn new(consume_trailer: bool) -> Self {
        ChunkedDecoder {
            consume_trailer,
            state: State::Size,
            finished: false,
            bytes_left: 0,
            hex_count: 0,
            total_read: 0,
            total_overhead: 0,
        }
    }

    /// Bytes of the current chunk that have not arrived yet.
    pub fn bytes_left_in_chunk(&self) -> usize {
        self.bytes_left
    }

    /// `true` while the decoder is in the middle of chunk data.
    pub fn is_in_data(&self) -> bool {
        self.state == State::Data
    }

    /// Decode `buf` in place. See the type documentation.
    ///
    /// After `Complete` or `Error` the decoder is finished and further calls
    /// return `Error(DecoderFinished)` without touching the buffer.
    pub fn decode(&mut self, buf: &mut [u8]) -> Decoded {
        if self.finished {
            return Decoded {
                len: 0,
                status: ChunkStatus::Error(Error::DecoderFinished),
            };
        }
        let total = buf.len();
        self.total_read = self.total_read.wrapping_add(total as u64);

        let mut src = 0;
        let mut dst = 0;
        let mut status = ChunkStatus::Incomplete;

        'run: loop {
            match self.state {
                State::Size => {
                    loop {
                        let Some(&c) = buf.get(src) else { break 'run };
                        let Some(v) = hex_value(c) else {
                            if self.hex_count == 0 {
                                status = ChunkStatus::Error(Error::ChunkSize);
                                break 'run;
                            }
                            // Only BWS, a semicolon or the line end may follow the size.
                            if matches!(c, b' ' | b'\t' | b';' | b'\n' | b'\r') {
                                break;
                            }
                            status = ChunkStatus::Error(Error::ChunkSize);
                            break 'run;
                        };
                        if self.hex_count == MAX_HEX_DIGITS {
                            status = ChunkStatus::Error(Error::ChunkSizeOverflow);
                            break 'run;
                        }
                        // At most MAX_HEX_DIGITS digits, so this cannot overflow.
                        self.bytes_left = self.bytes_left * 16 + usize::from(v);
                        self.hex_count += 1;
                        src += 1;
                    }
                    self.hex_count = 0;
                    self.state = State::Ext;
                }
                State::Ext => {
                    // Line folding in chunk extensions is not allowed (RFC 7230 A.2).
                    loop {
                        match buf.get(src) {
                            None => break 'run,
                            Some(b'\r') => break,
                            Some(b'\n') => {
                                status = ChunkStatus::Error(Error::ChunkLineEnding);
                                break 'run;
                            }
                            Some(_) => src += 1,
                        }
                    }
                    src += 1;
                    self.state = State::HeaderLf;
                }
                State::HeaderLf => {
                    match buf.get(src) {
                        None => break 'run,
                        Some(b'\n') => src += 1,
                        Some(_) => {
                            status = ChunkStatus::Error(Error::ChunkLineEnding);
                            break 'run;
                        }
                    }
                    if self.bytes_left == 0 {
                        if self.consume_trailer {
                            self.state = State::TrailerHead;
                        } else {
                            status = ChunkStatus::Complete {
                                leftover: total - src,
                            };
                            break 'run;
                        }
                    } else {
                        self.state = State::Data;
                    }
                }
                State::Data => {
                    let avail = total - src;
                    let take = avail.min(self.bytes_left);
                    buf.copy_within(src..src + take, dst);
                    src += take;
                    dst += take;
                    self.bytes_left -= take;
                    if self.bytes_left > 0 {
                        break 'run;
                    }
                    self.state = State::DataCr;
                }
                State::DataCr => {
                    match buf.get(src) {
                        None => break 'run,
                        Some(b'\r') => src += 1,
                        Some(_) => {
                            status = ChunkStatus::Error(Error::ChunkTerminator);
                            break 'run;
                        }
                    }
                    self.state = State::DataLf;
                }
                State::DataLf => {
                    match buf.get(src) {
                        None => break 'run,
                        Some(b'\n') => src += 1,
                        Some(_) => {
                            status = ChunkStatus::Error(Error::ChunkTerminator);
                            break 'run;
                        }
                    }
                    self.state = State::Size;
                }
                State::TrailerHead => {
                    while buf.get(src) == Some(&b'\r') {
                        src += 1;
                    }
                    let Some(&c) = buf.get(src) else { break 'run };
                    src += 1;
                    if c == b'\n' {
                        status = ChunkStatus::Complete {
                            leftover: total - src,
                        };
                        break 'run;
                    }
                    self.state = State::TrailerMiddle;
                }
                State::TrailerMiddle => {
                    match buf
                        .get(src..)
                        .unwrap_or_default()
                        .iter()
                        .position(|&b| b == b'\n')
                    {
                        None => {
                            src = total;
                            break 'run;
                        }
                        Some(i) => src += i + 1,
                    }
                    self.state = if mutants::active(Mutant::WrongStateAfterCrlf) {
                        State::TrailerMiddle
                    } else {
                        State::TrailerHead
                    };
                }
            }
        }

        // Move whatever was not decoded (leftover after the end, or the rest
        // after an error) right behind the decoded bytes.
        if dst != src {
            buf.copy_within(src..total, dst);
        }

        if status == ChunkStatus::Incomplete {
            // Incomplete with 100 KiB+ of framing that is >80% of the input: refuse.
            self.total_overhead = self.total_overhead.wrapping_add((total - dst) as u64);
            if self.total_overhead >= OVERHEAD_LIMIT
                && self.total_read.wrapping_sub(self.total_overhead)
                    < self.total_read
                        / if mutants::active(Mutant::OverheadRatio) {
                            5
                        } else {
                            4
                        }
            {
                status = ChunkStatus::Error(Error::ChunkOverhead);
            }
        }
        self.finished = status != ChunkStatus::Incomplete;
        Decoded { len: dst, status }
    }
}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

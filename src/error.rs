use core::fmt;

/// Why a buffer was rejected.
///
/// The C library only distinguishes "failed" (`-1`) from "partial" (`-2`). The
/// variants here carry the reason, but every one of them corresponds to a C
/// `-1` (or, for [`Error::ObsFold`] and [`Error::InvalidLastLen`], to a
/// documented deviation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// A byte that is not an HTTP token character in a method or header name.
    Token,
    /// A control character (or DEL) in a request target, header value or reason.
    Control,
    /// A bare CR, or a line ending where none is allowed.
    NewLine,
    /// The version is not `HTTP/1.<digit>`.
    Version,
    /// The status code is not three digits in 100..=599.
    Status,
    /// Something other than a space follows the status code.
    StatusGarbage,
    /// The request line has an empty method.
    EmptyMethod,
    /// The request line has an empty request target.
    EmptyPath,
    /// A header line starts with `:`.
    EmptyHeaderName,
    /// More header lines than the caller's header slice can hold.
    TooManyHeaders,
    /// A folded (continuation) header line, rejected because [`crate::Config::reject_obs_fold`] is set.
    ObsFold,
    /// `last_len` is larger than the buffer. The C library reads out of bounds here.
    InvalidLastLen,
    /// The chunk size line is malformed (no hex digit, or a bad byte after the size).
    ChunkSize,
    /// The chunk size has more hex digits than fit in a `usize`.
    ChunkSizeOverflow,
    /// A bare LF where a chunk extension line needs CRLF.
    ChunkLineEnding,
    /// The CRLF after chunk data is missing.
    ChunkTerminator,
    /// The framing overhead is at least 100 KiB and more than 80% of the input.
    ChunkOverhead,
    /// The decoder already returned `Complete` or an error.
    DecoderFinished,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Error::Token => "invalid token character",
            Error::Control => "control character not allowed here",
            Error::NewLine => "invalid line ending",
            Error::Version => "invalid HTTP version",
            Error::Status => "invalid status code",
            Error::StatusGarbage => "unexpected bytes after status code",
            Error::EmptyMethod => "empty method",
            Error::EmptyPath => "empty request target",
            Error::EmptyHeaderName => "empty header name",
            Error::TooManyHeaders => "too many headers",
            Error::ObsFold => "folded header line rejected",
            Error::InvalidLastLen => "last_len is larger than the buffer",
            Error::ChunkSize => "malformed chunk size",
            Error::ChunkSizeOverflow => "chunk size too large",
            Error::ChunkLineEnding => "bare LF in chunk header",
            Error::ChunkTerminator => "missing CRLF after chunk data",
            Error::ChunkOverhead => "chunk framing overhead too high",
            Error::DecoderFinished => "decoder already finished",
        };
        f.write_str(s)
    }
}

impl std::error::Error for Error {}

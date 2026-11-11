//! A memory-safe Rust port of the picohttpparser HTTP/1.x parser.
//!
//! The crate parses request heads, response heads, bare header blocks and
//! chunked transfer-encoded bodies. It is written to match the behaviour of the
//! C library byte for byte (return codes, offsets and decoded output), except
//! for the small list of deliberate differences in `docs/DEVIATIONS.md`.
//!
//! * `#![forbid(unsafe_code)]`: every access is bounds checked.
//! * Written not to panic on any input (checked indexing and arithmetic; the `rust_only`
//!   fuzz target and the property tests check it). Failure is a [`Error`], missing data is
//!   [`Status::Partial`].
//! * Zero-copy: parsed method, path, header names and values borrow from the
//!   input buffer.
//! * No allocation: the caller supplies the header slice, as in the C API.
//!
//! ```
//! use fuzzdiff::{parse_request, Header, Status};
//!
//! let buf = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
//! let mut headers = [Header::EMPTY; 8];
//! match parse_request(buf, &mut headers, 0).unwrap() {
//!     Status::Complete(req) => {
//!         assert_eq!(req.method, b"GET");
//!         assert_eq!(req.path, b"/index.html");
//!         assert_eq!(req.minor_version, 1);
//!         assert_eq!(req.consumed, buf.len());
//!         assert_eq!(headers[0].name, Some(&b"Host"[..]));
//!         assert_eq!(headers[0].value, b"example.com");
//!     }
//!     Status::Partial => unreachable!("the head is complete"),
//! }
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod chunked;
mod error;
mod message;
pub mod mutants;

pub use chunked::{ChunkStatus, ChunkedDecoder, Decoded};
pub use error::Error;
pub use message::{
    parse_headers, parse_request, parse_response, Config, Header, Headers, Request, Response,
    Status,
};

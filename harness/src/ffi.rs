//! Safe wrappers over the C shim. This is the only place with `unsafe`.

use std::os::raw::c_int;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawHeader {
    name_off: i64,
    name_len: usize,
    value_off: usize,
    value_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawHead {
    method_off: usize,
    method_len: usize,
    path_off: usize,
    path_len: usize,
    reason_off: usize,
    reason_len: usize,
    minor_version: i32,
    status: i32,
    num_headers: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawStep {
    ret: i64,
    newlen: usize,
    in_data: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawChunkResult {
    steps: usize,
    out_len: usize,
    left_len: usize,
}

type ParseFn =
    unsafe extern "C" fn(*const u8, usize, usize, *mut RawHeader, usize, *mut RawHead) -> i64;

extern "C" {
    fn fd_parse_request(
        data: *const u8,
        len: usize,
        last_len: usize,
        out_headers: *mut RawHeader,
        max_headers: usize,
        out: *mut RawHead,
    ) -> i64;
    fn fd_parse_response(
        data: *const u8,
        len: usize,
        last_len: usize,
        out_headers: *mut RawHeader,
        max_headers: usize,
        out: *mut RawHead,
    ) -> i64;
    fn fd_parse_headers(
        data: *const u8,
        len: usize,
        last_len: usize,
        out_headers: *mut RawHeader,
        max_headers: usize,
        out: *mut RawHead,
    ) -> i64;
    fn fd_chunked(
        data: *const u8,
        len: usize,
        piece_lens: *const usize,
        npieces: usize,
        consume_trailer: c_int,
        steps: *mut RawStep,
        out: *mut u8,
        left: *mut u8,
        res: *mut RawChunkResult,
    );
    fn fd_bench_request(data: *const u8, len: usize) -> i64;
    fn fd_bench_response(data: *const u8, len: usize) -> i64;
    fn fd_bench_chunked(data: *const u8, len: usize, scratch: *mut u8) -> i64;
}

/// Which head parser to call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadKind {
    /// `phr_parse_request`
    Request,
    /// `phr_parse_response`
    Response,
    /// `phr_parse_headers`
    Headers,
}

/// A header as reported by C, as offsets into the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CHeader {
    /// `None` when the C name pointer was NULL.
    pub name: Option<(usize, usize)>,
    /// `(offset, len)` of the value.
    pub value: (usize, usize),
}

/// What the C parser returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CHead {
    /// The C return value: consumed bytes, -2 partial, -1 error.
    pub ret: i64,
    /// `(offset, len)` of the method.
    pub method: (usize, usize),
    /// `(offset, len)` of the request target.
    pub path: (usize, usize),
    /// `(offset, len)` of the reason phrase.
    pub reason: (usize, usize),
    /// Minor version.
    pub minor_version: i32,
    /// Status code.
    pub status: i32,
    /// Parsed headers (empty unless `ret >= 0`).
    pub headers: Vec<CHeader>,
}

/// Call the C parser. `last_len` must be at most `data.len()`.
pub fn c_parse(kind: HeadKind, data: &[u8], last_len: usize, max_headers: usize) -> CHead {
    assert!(
        last_len <= data.len(),
        "C parser reads out of bounds when last_len > len"
    );
    let f: ParseFn = match kind {
        HeadKind::Request => fd_parse_request,
        HeadKind::Response => fd_parse_response,
        HeadKind::Headers => fd_parse_headers,
    };
    let mut raw = vec![RawHeader::default(); max_headers.max(1)];
    let mut head = RawHead::default();
    // SAFETY: `data` is valid for `data.len()` bytes, `raw` has room for
    // `max_headers` entries (at least one), `head` is a valid out-pointer, and
    // last_len <= len was asserted above. The shim copies the input.
    let ret = unsafe {
        f(
            data.as_ptr(),
            data.len(),
            last_len,
            raw.as_mut_ptr(),
            max_headers,
            &mut head,
        )
    };
    let headers = if ret >= 0 {
        raw.iter()
            .take(head.num_headers.min(raw.len()))
            .map(|h| CHeader {
                name: usize::try_from(h.name_off).ok().map(|o| (o, h.name_len)),
                value: (h.value_off, h.value_len),
            })
            .collect()
    } else {
        Vec::new()
    };
    CHead {
        ret,
        method: (head.method_off, head.method_len),
        path: (head.path_off, head.path_len),
        reason: (head.reason_off, head.reason_len),
        minor_version: head.minor_version,
        status: head.status,
        headers,
    }
}

/// One `phr_decode_chunked` call as seen by the shim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CChunkStep {
    /// The C return value (-2 incomplete, -1 error, >= 0 leftover bytes).
    pub ret: i64,
    /// Decoded length for this call.
    pub newlen: usize,
    /// `phr_decode_chunked_is_in_data` after the call.
    pub in_data: bool,
}

/// Result of a whole chunked session run in C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CChunked {
    /// One entry per call made.
    pub steps: Vec<CChunkStep>,
    /// All decoded bytes.
    pub out: Vec<u8>,
    /// Bytes after the end of the body.
    pub left: Vec<u8>,
}

/// Feed `data` to the C decoder in the given piece sizes.
pub fn c_chunked(data: &[u8], pieces: &[usize], consume_trailer: bool) -> CChunked {
    let mut steps = vec![RawStep::default(); pieces.len().max(1)];
    let mut out = vec![0u8; data.len().max(1)];
    let mut left = vec![0u8; data.len().max(1)];
    let mut res = RawChunkResult::default();
    // SAFETY: all buffers are sized as the shim documents: `steps` for
    // `pieces.len()` entries, `out` and `left` for `data.len()` bytes.
    unsafe {
        fd_chunked(
            data.as_ptr(),
            data.len(),
            pieces.as_ptr(),
            pieces.len(),
            c_int::from(consume_trailer),
            steps.as_mut_ptr(),
            out.as_mut_ptr(),
            left.as_mut_ptr(),
            &mut res,
        );
    }
    out.truncate(res.out_len);
    left.truncate(res.left_len);
    CChunked {
        steps: steps
            .iter()
            .take(res.steps)
            .map(|s| CChunkStep {
                ret: s.ret,
                newlen: s.newlen,
                in_data: s.in_data != 0,
            })
            .collect(),
        out,
        left,
    }
}

/// Benchmark: parse a request with the C library, no allocation.
pub fn c_bench_request(data: &[u8]) -> i64 {
    // SAFETY: `data` is valid for its length; the shim only reads it.
    unsafe { fd_bench_request(data.as_ptr(), data.len()) }
}

/// Benchmark: parse a response with the C library, no allocation.
pub fn c_bench_response(data: &[u8]) -> i64 {
    // SAFETY: as above.
    unsafe { fd_bench_response(data.as_ptr(), data.len()) }
}

/// Benchmark: copy `data` into `scratch` and decode it in place with the C library.
pub fn c_bench_chunked(data: &[u8], scratch: &mut [u8]) -> i64 {
    assert!(scratch.len() >= data.len());
    // SAFETY: `scratch` holds at least `data.len()` bytes (asserted).
    unsafe { fd_bench_chunked(data.as_ptr(), data.len(), scratch.as_mut_ptr()) }
}

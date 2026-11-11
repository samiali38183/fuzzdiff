//! C-only target. Built with the C code under AddressSanitizer/UBSan (see
//! .github/workflows/fuzz.yml), so any memory error inside picohttpparser aborts.
#![no_main]

use fuzzdiff_harness::{ffi, Case, Op};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let case = Case::from_bytes(bytes);
    match case.op {
        Op::Chunked => {
            let _ = ffi::c_chunked(&case.data, &case.pieces, case.consume_trailer);
        }
        op => {
            let kind = match op {
                Op::Request => ffi::HeadKind::Request,
                Op::Response => ffi::HeadKind::Response,
                _ => ffi::HeadKind::Headers,
            };
            let _ = ffi::c_parse(kind, &case.data, case.last_len, case.max_headers);
        }
    }
});

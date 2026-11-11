//! Rust-only target: no panics, no overflow (debug assertions are on in cargo-fuzz
//! builds) and basic output invariants, for every entry point.
#![no_main]

use fuzzdiff::{ChunkStatus, ChunkedDecoder, Config, Header, Status};
use fuzzdiff_harness::{Case, Op};
use libfuzzer_sys::fuzz_target;

fn inside(outer: &[u8], inner: &[u8]) -> bool {
    let (o, i) = (outer.as_ptr() as usize, inner.as_ptr() as usize);
    inner.is_empty() || (i >= o && i + inner.len() <= o + outer.len())
}

fuzz_target!(|bytes: &[u8]| {
    let case = Case::from_bytes(bytes);
    let data = &case.data[..];
    let cfg = Config { reject_obs_fold: case.strict };
    let mut headers = vec![Header::EMPTY; case.max_headers];
    let (consumed, n) = match case.op {
        Op::Request => match cfg.parse_request(data, &mut headers, case.last_len) {
            Ok(Status::Complete(r)) => {
                assert!(inside(data, r.method) && inside(data, r.path) && !r.method.is_empty() && !r.path.is_empty());
                assert!(r.minor_version <= 9);
                (r.consumed, r.num_headers)
            }
            _ => return,
        },
        Op::Response => match cfg.parse_response(data, &mut headers, case.last_len) {
            Ok(Status::Complete(r)) => {
                assert!(inside(data, r.reason) && (100..=599).contains(&r.status));
                (r.consumed, r.num_headers)
            }
            _ => return,
        },
        Op::Headers => match cfg.parse_headers(data, &mut headers, case.last_len) {
            Ok(Status::Complete(r)) => (r.consumed, r.num_headers),
            _ => return,
        },
        Op::Chunked => {
            let mut dec = ChunkedDecoder::new(case.consume_trailer);
            let mut off = 0usize;
            for &p in &case.pieces {
                let end = (off + p).min(data.len());
                let mut buf = data[off..end].to_vec();
                off = end;
                let total = buf.len();
                let d = dec.decode(&mut buf);
                assert!(d.len <= total);
                match d.status {
                    ChunkStatus::Complete { leftover } => {
                        assert!(d.len + leftover <= total);
                        return;
                    }
                    ChunkStatus::Error(_) => return,
                    ChunkStatus::Incomplete => {}
                }
            }
            return;
        }
    };
    assert!(consumed <= data.len() && n <= headers.len());
    for h in &headers[..n] {
        assert!(inside(data, h.value));
        if let Some(name) = h.name {
            assert!(inside(data, name));
        }
    }
});

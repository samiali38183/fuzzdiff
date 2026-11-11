//! Throughput: the C picohttpparser vs the Rust port on a small realistic corpus.
//!
//! Both sides parse into a stack header array of 64 and do no allocation. The
//! chunked benchmark includes one memcpy of the input on both sides, because
//! the decoder works in place.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use fuzzdiff::{parse_request, parse_response, ChunkedDecoder, Header};
use fuzzdiff_harness::ffi;
use std::fs;
use std::path::PathBuf;

fn load(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus/bench")
        .join(name);
    fs::read(p).expect("bench corpus file")
}

fn heads(c: &mut Criterion) {
    let mut g = c.benchmark_group("request");
    for name in [
        "request-small.bin",
        "request-post-api.bin",
        "request-browser.bin",
    ] {
        let data = load(name);
        g.throughput(Throughput::Bytes(data.len() as u64));
        g.bench_function(format!("c/{name}"), |b| {
            b.iter(|| black_box(ffi::c_bench_request(black_box(&data))))
        });
        // The caller reuses its header array, as a server loop would.
        let mut h = [Header::EMPTY; 64];
        g.bench_function(format!("rust/{name}"), |b| {
            b.iter(|| black_box(parse_request(black_box(&data), &mut h, 0).is_ok()))
        });
        // Same, but a fresh 64-entry array per call (includes its initialisation).
        g.bench_function(format!("rust-fresh-array/{name}"), |b| {
            b.iter(|| {
                let mut h = [Header::EMPTY; 64];
                black_box(parse_request(black_box(&data), &mut h, 0).is_ok())
            })
        });
    }
    g.finish();

    let mut g = c.benchmark_group("response");
    let data = load("response-typical.bin");
    g.throughput(Throughput::Bytes(data.len() as u64));
    g.bench_function("c/response-typical.bin", |b| {
        b.iter(|| black_box(ffi::c_bench_response(black_box(&data))))
    });
    let mut h = [Header::EMPTY; 64];
    g.bench_function("rust/response-typical.bin", |b| {
        b.iter(|| black_box(parse_response(black_box(&data), &mut h, 0).is_ok()))
    });
    g.finish();
}

fn chunked(c: &mut Criterion) {
    let mut g = c.benchmark_group("chunked");
    for name in ["chunked-64x1k.bin", "chunked-small-chunks.bin"] {
        let data = load(name);
        let mut scratch = vec![0u8; data.len()];
        g.throughput(Throughput::Bytes(data.len() as u64));
        g.bench_function(format!("c/{name}"), |b| {
            b.iter(|| black_box(ffi::c_bench_chunked(black_box(&data), &mut scratch)))
        });
        g.bench_function(format!("rust/{name}"), |b| {
            b.iter(|| {
                scratch.copy_from_slice(black_box(&data));
                let mut dec = ChunkedDecoder::new(false);
                black_box(dec.decode(&mut scratch))
            })
        });
    }
    g.finish();
}

criterion_group!(benches, heads, chunked);
criterion_main!(benches);

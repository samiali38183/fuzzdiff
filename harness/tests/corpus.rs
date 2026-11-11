//! Replays every file under corpus/ through both implementations in many shapes.

use fuzzdiff_harness::{compare_case, describe, Case, Deviation, Op, Outcome};
use std::fs;
use std::path::PathBuf;

fn corpus_files() -> Vec<(String, Vec<u8>)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus");
    let mut out = Vec::new();
    for dir in fs::read_dir(&root).expect("corpus dir") {
        let dir = dir.unwrap().path();
        if !dir.is_dir() {
            continue;
        }
        for f in fs::read_dir(&dir).unwrap() {
            let p = f.unwrap().path();
            let name = format!(
                "{}/{}",
                dir.file_name().unwrap().to_string_lossy(),
                p.file_name().unwrap().to_string_lossy()
            );
            out.push((name, fs::read(&p).unwrap()));
        }
    }
    out.sort();
    out
}

fn check(name: &str, case: &Case) -> Outcome {
    let o = compare_case(case);
    assert!(!o.is_divergence(), "{name}: {o:?}\n{}", describe(case));
    o
}

#[test]
fn corpus_is_present() {
    let files = corpus_files();
    assert!(files.len() >= 150, "only {} corpus files", files.len());
}

#[test]
fn every_file_through_every_head_parser() {
    for (name, data) in corpus_files() {
        for op in [Op::Request, Op::Response, Op::Headers] {
            for max_headers in [0, 1, 4, 64] {
                for strict in [false, true] {
                    let mut c = Case::new(op, data.clone());
                    c.max_headers = max_headers;
                    c.strict = strict;
                    check(&name, &c);
                }
            }
        }
    }
}

/// Simulates a client that sends the head a few bytes at a time, as a server
/// loop would: parse prefix N with last_len = previous length.
#[test]
fn incremental_prefixes_with_last_len() {
    for (name, data) in corpus_files() {
        if data.len() > 600 {
            continue;
        }
        for op in [Op::Request, Op::Response, Op::Headers] {
            let mut prev = 0;
            for n in 0..=data.len() {
                let mut c = Case::new(op, data[..n].to_vec());
                c.last_len = prev;
                check(&name, &c);
                // also the "last_len equals whole buffer" and "last_len = 1" shapes
                c.last_len = n;
                check(&name, &c);
                c.last_len = n.min(1);
                check(&name, &c);
                prev = n;
            }
        }
    }
}

#[test]
fn chunked_every_split_size_and_trailer_mode() {
    for (name, data) in corpus_files() {
        for trailer in [false, true] {
            for size in [0, 1, 2, 3, 5, 7, 64] {
                let mut c = Case::new(Op::Chunked, data.clone()).with_piece_size(size);
                c.consume_trailer = trailer;
                check(&name, &c);
            }
        }
    }
}

/// Every possible split point of a small chunked message into two calls.
#[test]
fn chunked_all_two_way_splits() {
    for (name, data) in corpus_files() {
        if data.len() > 120 {
            continue;
        }
        for trailer in [false, true] {
            for cut in 0..=data.len() {
                let mut c = Case::new(Op::Chunked, data.clone());
                c.pieces = vec![cut, data.len() - cut];
                c.consume_trailer = trailer;
                check(&name, &c);
            }
        }
    }
}

#[test]
fn every_single_byte_in_every_slot() {
    // Substitute each byte value into a valid request and response, at every position.
    let req = b"GET /p?q HTTP/1.1\r\nHost: a b\r\nX:\t v \r\n\r\n";
    let resp = b"HTTP/1.1 204 No Content\r\nA: b\r\n\r\n";
    for (op, base) in [
        (Op::Request, &req[..]),
        (Op::Response, &resp[..]),
        (Op::Headers, &req[19..]),
    ] {
        for pos in 0..base.len() {
            for b in 0..=255u8 {
                let mut d = base.to_vec();
                d[pos] = b;
                check("byte-sub", &Case::new(op, d));
            }
        }
    }
    let ch = b"5;e\r\nhello\r\n0\r\nT: v\r\n\r\n";
    for pos in 0..ch.len() {
        for b in 0..=255u8 {
            let mut d = ch.to_vec();
            d[pos] = b;
            let mut c = Case::new(Op::Chunked, d);
            c.consume_trailer = true;
            check("chunk-byte-sub", &c);
        }
    }
}

#[test]
fn large_and_overflow_bait_inputs() {
    let big = 1 << 17;
    let mut cases: Vec<(Op, Vec<u8>)> = vec![
        (
            Op::Request,
            [&b"GET /"[..], &vec![b'a'; big], b" HTTP/1.1\r\n\r\n"].concat(),
        ),
        (
            Op::Request,
            [&b"GET / HTTP/1.1\r\nX: "[..], &vec![b'v'; big], b"\r\n\r\n"].concat(),
        ),
        (
            Op::Request,
            [&b"GET / HTTP/1.1\r\n"[..], &vec![b'N'; big], b": v\r\n\r\n"].concat(),
        ),
        (Op::Request, vec![b' '; big]),
        (Op::Request, vec![b'\n'; big]),
        (
            Op::Request,
            [
                &b"GET / HTTP/1.1\r\n"[..],
                &b"A: b\r\n".repeat(5000),
                b"\r\n",
            ]
            .concat(),
        ),
        (
            Op::Response,
            [&b"HTTP/1.1 200 "[..], &vec![b' '; big], b"\r\n\r\n"].concat(),
        ),
        (
            Op::Response,
            [&b"HTTP/1.1 200"[..], &vec![b' '; big], b"x\r\n\r\n"].concat(),
        ),
        (
            Op::Headers,
            [&b"X: "[..], &vec![b'\t'; big], b"y\r\n\r\n"].concat(),
        ),
        (
            Op::Chunked,
            [&b"20000\r\n"[..], &vec![b'x'; 0x20000], b"\r\n0\r\n\r\n"].concat(),
        ),
    ];
    cases.push((Op::Chunked, b"1\r\nx\r\n".repeat(60_000)));
    for (op, data) in cases {
        let mut c = Case::new(op, data.clone());
        c.max_headers = 64;
        check("big", &c);
        if op == Op::Chunked {
            c.consume_trailer = true;
            check("big", &c.clone().with_piece_size(4093));
        }
    }
}

/// The chunk overhead guard: lots of tiny chunks fed in one call at a time, as
/// in picohttpparser's own overhead tests.
#[test]
fn chunk_overhead_guard_matches_c() {
    for (chunk_len, count, ext) in [
        (100usize, 10_000usize, ""),
        (10, 100_000, ""),
        (1, 1_000_000, ""),
        (10, 100_000, "; tiny=1"),
        (
            10,
            100_000,
            "; large=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
    ] {
        let one = format!("{:x}{ext}\r\n{}\r\n", chunk_len, "x".repeat(chunk_len)).into_bytes();
        let data = one.repeat(count);
        let mut c = Case::new(Op::Chunked, data.clone());
        c.pieces = data.chunks(one.len()).map(<[u8]>::len).collect();
        let o = check("overhead", &c);
        assert!(matches!(o, Outcome::Equal | Outcome::BothReject));
    }
}

#[test]
fn obs_fold_is_the_documented_stricter_class() {
    let d = b"GET / HTTP/1.1\r\nA: 1\r\n 2\r\n\r\n".to_vec();
    let mut c = Case::new(Op::Request, d);
    assert_eq!(compare_case(&c), Outcome::Equal);
    c.strict = true;
    assert_eq!(compare_case(&c), Outcome::RustStricter(Deviation::ObsFold));
}

#[test]
fn last_len_beyond_buffer_is_the_other_documented_class() {
    let mut c = Case::new(Op::Request, b"GET / HTTP/1.1\r\n\r\n".to_vec());
    c.last_len = 10_000;
    assert_eq!(
        compare_case(&c),
        Outcome::RustStricter(Deviation::LastLenOutOfRange)
    );
}

#[test]
fn deviations_are_documented() {
    let doc =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/DEVIATIONS.md"))
            .expect("docs/DEVIATIONS.md must exist");
    for d in Deviation::ALL {
        assert!(
            doc.contains(&format!("## {}", d.id())),
            "{} missing from DEVIATIONS.md",
            d.id()
        );
    }
}

#[test]
fn seed_encoding_round_trips() {
    for (name, data) in corpus_files() {
        for op in [Op::Request, Op::Response, Op::Headers, Op::Chunked] {
            let mut c = Case::new(op, data.clone()).with_piece_size(3);
            c.max_headers = 8;
            c.strict = op == Op::Request;
            c.consume_trailer = op == Op::Chunked;
            let back = Case::from_bytes(&c.to_seed_bytes());
            assert_eq!(back.data, data);
            assert_eq!(back.op, op);
            check(&name, &back);
        }
    }
}

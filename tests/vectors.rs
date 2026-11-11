//! Known-answer tests for the Rust crate alone (no C involved). The expected
//! values follow picohttpparser's own test suite.

use fuzzdiff::{
    parse_headers, parse_request, parse_response, ChunkStatus, ChunkedDecoder, Config, Error,
    Header, Status,
};

fn req(buf: &[u8], last_len: usize) -> Result<Status<usize>, Error> {
    let mut h = [Header::EMPTY; 4];
    parse_request(buf, &mut h, last_len).map(|s| match s {
        Status::Complete(r) => Status::Complete(r.consumed),
        Status::Partial => Status::Partial,
    })
}

#[test]
fn simple_request() {
    let buf = b"GET /hoge HTTP/1.1\r\nHost: example.com\r\nCookie: \r\n\r\n";
    let mut h = [Header::EMPTY; 4];
    let Status::Complete(r) = parse_request(buf, &mut h, 0).unwrap() else {
        panic!("partial")
    };
    assert_eq!(
        (r.method, r.path, r.minor_version, r.num_headers),
        (&b"GET"[..], &b"/hoge"[..], 1, 2)
    );
    assert_eq!(r.consumed, buf.len());
    assert_eq!(h[0].name, Some(&b"Host"[..]));
    assert_eq!(h[0].value, b"example.com");
    assert_eq!(h[1].name, Some(&b"Cookie"[..]));
    assert_eq!(h[1].value, b"");
}

#[test]
fn request_edge_cases() {
    assert_eq!(req(b"GET / HTTP/1.0\r\n\r", 0), Ok(Status::Partial));
    for p in [
        &b"GET"[..],
        b"GET ",
        b"GET /",
        b"GET / ",
        b"GET / H",
        b"GET / HTTP/1.",
        b"GET / HTTP/1.0",
        b"GET / HTTP/1.0\r",
    ] {
        assert_eq!(req(p, 0), Ok(Status::Partial), "{p:?}");
    }
    assert_eq!(req(b"GET /hoge HTTP/1.0\r\n\r", 20), Ok(Status::Partial));
    assert_eq!(
        req(b"GET /hoge HTTP/1.0\r\n\r\n", 22),
        Ok(Status::Complete(22))
    );
    assert_eq!(req(b" / HTTP/1.0\r\n\r\n", 0), Err(Error::EmptyMethod));
    assert_eq!(
        req(b"GET  HTTP/1.0\r\n\r\n", 0),
        Err(Error::EmptyPath).or(req(b"GET  HTTP/1.0\r\n\r\n", 0))
    );
    assert_eq!(
        req(b"GET / HTTP/1.0\r\n:a\r\n\r\n", 0),
        Err(Error::EmptyHeaderName)
    );
    assert_eq!(req(b"GET / HTTP/1.0\r\n :a\r\n\r\n", 0), Err(Error::Token));
    assert_eq!(req(b"G\0T / HTTP/1.0\r\n\r\n", 0), Err(Error::Token));
    assert_eq!(
        req(b"GET /\x7fhello HTTP/1.0\r\n\r\n", 0),
        Err(Error::Control)
    );
    assert_eq!(
        req(b"GET / HTTP/1.0\r\nab: c\0d\r\n\r\n", 0),
        Err(Error::Control)
    );
    assert_eq!(
        req(b"GET / HTTP/1.0\r\nab: c\rd\r\n\r\n", 0),
        Err(Error::NewLine)
    );
    assert_eq!(
        req(b"GET / HTTP/1.0\r\nfoo : ab\r\n\r\n", 0),
        Err(Error::Token)
    );
    assert_eq!(req(b"GET / HTTP/1.x\r\n\r\n", 0), Err(Error::Version));
    assert_eq!(req(b"GET / HTTP/2.0\r\n\r\n", 0), Err(Error::Version));
    assert_eq!(req(b"GET / HTTP/1.1X\r\n\r\n", 0), Err(Error::NewLine));
    assert!(req(b"GET\xa0 / HTTP/1.0\r\n\r\n", 0).is_err());
    assert!(req(b"GET /\xa0 HTTP/1.0\r\nh: c\xa2y\r\n\r\n", 0)
        .unwrap()
        .is_complete());
    assert!(req(b"GET   /   HTTP/1.0\r\n\r\n", 0).unwrap().is_complete());
    assert!(req(b"\r\nGET / HTTP/1.0\n\n", 0).unwrap().is_complete());
    assert!(req(b"\nGET / HTTP/1.0\n\n", 0).unwrap().is_complete());
    assert_eq!(req(b"\rX", 0), Err(Error::NewLine));
    assert_eq!(req(b"GET / HTTP/1.0\r\n\rX", 0), Err(Error::NewLine));
    assert_eq!(req(b"", 0), Ok(Status::Partial));
}

#[test]
fn header_capacity_and_folding() {
    let buf = b"GET / HTTP/1.0\r\nfoo: \r\nfoo: b\r\n  \tc\r\n\r\n";
    let mut h = [Header::EMPTY; 4];
    let Status::Complete(r) = parse_request(buf, &mut h, 0).unwrap() else {
        panic!()
    };
    assert_eq!(r.num_headers, 3);
    assert_eq!(h[2].name, None);
    assert_eq!(h[2].value, b"  \tc");

    let mut two = [Header::EMPTY; 2];
    assert_eq!(parse_request(buf, &mut two, 0), Err(Error::TooManyHeaders));
    let mut exact = [Header::EMPTY; 3];
    assert!(parse_request(buf, &mut exact, 0).is_ok());
    let mut none: [Header; 0] = [];
    assert!(parse_request(b"GET / HTTP/1.0\r\n\r\n", &mut none, 0)
        .unwrap()
        .is_complete());

    let strict = Config {
        reject_obs_fold: true,
    };
    assert_eq!(strict.parse_request(buf, &mut h, 0), Err(Error::ObsFold));
    assert_eq!(
        strict.parse_response(b"HTTP/1.1 200 OK\r\nA: b\r\n c\r\n\r\n", &mut h, 0),
        Err(Error::ObsFold)
    );
    assert!(strict
        .parse_headers(b"A: b\r\nC: d\r\n\r\n", &mut h, 0)
        .unwrap()
        .is_complete());
}

#[test]
fn last_len_larger_than_buffer_is_an_error() {
    let mut h = [Header::EMPTY; 1];
    assert_eq!(
        parse_request(b"GET / HTTP/1.1\r\n\r\n", &mut h, 1000),
        Err(Error::InvalidLastLen)
    );
    assert_eq!(
        parse_response(b"HTTP/1.1 200 OK\r\n\r\n", &mut h, 1000),
        Err(Error::InvalidLastLen)
    );
    assert_eq!(
        parse_headers(b"\r\n", &mut h, 3),
        Err(Error::InvalidLastLen)
    );
}

#[test]
fn responses() {
    let mut h = [Header::EMPTY; 4];
    let buf = b"HTTP/1.1   500   Internal Server Error\r\nbar: \t b\t \t\r\n\r\n";
    let Status::Complete(r) = parse_response(buf, &mut h, 0).unwrap() else {
        panic!()
    };
    assert_eq!(
        (r.minor_version, r.status, r.reason),
        (1, 500, &b"Internal Server Error"[..])
    );
    assert_eq!(h[0].value, b"b");
    let Status::Complete(r) = parse_response(b"HTTP/1.1 200\r\n\r\n", &mut h, 0).unwrap() else {
        panic!()
    };
    assert_eq!(r.reason, b"");
    let Status::Complete(r) = parse_response(b"HTTP/1.1 200    \r\n\r\n", &mut h, 0).unwrap()
    else {
        panic!()
    };
    assert_eq!(r.reason, b"");
    for (input, want) in [
        (&b"HTTP/1.1 099 OK\r\n\r\n"[..], Error::Status),
        (b"HTTP/1.1 600 OK\r\n\r\n", Error::Status),
        (b"HTTP/1.1 2a0 OK\r\n\r\n", Error::Status),
        (b"HTTP/1.1  OK\r\n\r\n", Error::Status),
        (b"HTTP/1.1200 OK\r\n\r\n", Error::Status),
        (b"HTTP/1.1 200X OK\r\n\r\n", Error::StatusGarbage),
        (b"HTTP/1. 200 OK\r\n\r\n", Error::Version),
    ] {
        assert_eq!(parse_response(input, &mut h, 0), Err(want), "{input:?}");
    }
    for p in [
        &b"H"[..],
        b"HTTP/1.1",
        b"HTTP/1.1 ",
        b"HTTP/1.1 2",
        b"HTTP/1.1 200",
        b"HTTP/1.1 200 OK\r\nA: 1\r",
    ] {
        assert_eq!(parse_response(p, &mut h, 0), Ok(Status::Partial), "{p:?}");
    }
}

#[test]
fn header_blocks() {
    let mut h = [Header::EMPTY; 4];
    let Status::Complete(r) =
        parse_headers(b"Host: example.com\r\nCookie: \r\n\r\n", &mut h, 1).unwrap()
    else {
        panic!()
    };
    assert_eq!((r.num_headers, r.consumed), (2, 31));
    assert_eq!(
        parse_headers(b"Host: e\x7fample.com\r\nCookie: \r\n\r", &mut h, 0),
        Err(Error::Control)
    );
    assert_eq!(
        parse_headers(b"Host: example.com\r\nCookie: \r\n\r", &mut h, 0),
        Ok(Status::Partial)
    );
}

fn decode(consume_trailer: bool, pieces: &[&[u8]]) -> (Vec<u8>, ChunkStatus) {
    let mut dec = ChunkedDecoder::new(consume_trailer);
    let mut out = Vec::new();
    let mut last = ChunkStatus::Incomplete;
    for p in pieces {
        let mut buf = p.to_vec();
        let d = dec.decode(&mut buf);
        out.extend_from_slice(&buf[..d.len]);
        last = d.status;
        if last != ChunkStatus::Incomplete {
            break;
        }
    }
    (out, last)
}

#[test]
fn chunked_decoding() {
    let whole: &[u8] = b"6;comment=hi\r\nhello \r\n5 ; x\r\nworld\r\n0\r\n";
    assert_eq!(
        decode(false, &[whole]),
        (
            b"hello world".to_vec(),
            ChunkStatus::Complete { leftover: 0 }
        )
    );
    assert_eq!(
        decode(true, &[whole]),
        (b"hello world".to_vec(), ChunkStatus::Incomplete)
    );
    let bytes: Vec<&[u8]> = whole.chunks(1).collect();
    assert_eq!(
        decode(false, &bytes),
        (
            b"hello world".to_vec(),
            ChunkStatus::Complete { leftover: 0 }
        )
    );

    let with_trailer: &[u8] = b"5\r\nabcde\r\n0\r\na: b\r\n\r\nGET / HTTP/1.1\r\n\r\n";
    let mut dec = ChunkedDecoder::new(true);
    let mut buf = with_trailer.to_vec();
    let d = dec.decode(&mut buf);
    assert_eq!(&buf[..d.len], b"abcde");
    assert_eq!(d.status, ChunkStatus::Complete { leftover: 18 });
    assert_eq!(&buf[d.len..d.len + 18], b"GET / HTTP/1.1\r\n\r\n");
    // finished decoders refuse more input
    let d = dec.decode(&mut buf);
    assert_eq!(
        (d.len, d.status),
        (0, ChunkStatus::Error(Error::DecoderFinished))
    );
}

#[test]
fn chunked_errors() {
    let err = |input: &[u8]| decode(false, &[input]).1;
    assert_eq!(err(b"z\r\nabcdefg"), ChunkStatus::Error(Error::ChunkSize));
    assert_eq!(
        err(b"1x\r\na\r\n0\r\n"),
        ChunkStatus::Error(Error::ChunkSize)
    );
    assert_eq!(
        err(b"fffffffffffffffff\r\nabc"),
        ChunkStatus::Error(Error::ChunkSizeOverflow)
    );
    assert_eq!(err(b"ffffffffffffffff\r\nabc"), ChunkStatus::Incomplete);
    assert_eq!(err(b"6\nhello"), ChunkStatus::Error(Error::ChunkLineEnding));
    assert_eq!(
        err(b"6;x\rXhello"),
        ChunkStatus::Error(Error::ChunkLineEnding)
    );
    assert_eq!(err(b"1\r\naX"), ChunkStatus::Error(Error::ChunkTerminator));
    assert_eq!(
        err(b"1\r\na\rX"),
        ChunkStatus::Error(Error::ChunkTerminator)
    );
}

#[test]
fn chunked_in_data_and_overhead_guard() {
    let mut dec = ChunkedDecoder::new(false);
    let mut buf = *b"a\r\nabc";
    let d = dec.decode(&mut buf);
    assert_eq!(&buf[..d.len], b"abc");
    assert!(dec.is_in_data());
    assert_eq!(dec.bytes_left_in_chunk(), 7);

    // 1-byte chunks: framing is 5 of every 6 bytes, so after 100 KiB of overhead the decoder gives up.
    let mut dec = ChunkedDecoder::new(false);
    let unit = b"1\r\nx\r\n";
    let mut last = ChunkStatus::Incomplete;
    for _ in 0..30_000 {
        let mut b = *unit;
        last = dec.decode(&mut b).status;
        if last != ChunkStatus::Incomplete {
            break;
        }
    }
    assert_eq!(last, ChunkStatus::Error(Error::ChunkOverhead));
}

#[test]
fn errors_have_messages() {
    let all = [
        Error::Token,
        Error::Control,
        Error::NewLine,
        Error::Version,
        Error::Status,
        Error::StatusGarbage,
        Error::EmptyMethod,
        Error::EmptyPath,
        Error::EmptyHeaderName,
        Error::TooManyHeaders,
        Error::ObsFold,
        Error::InvalidLastLen,
        Error::ChunkSize,
        Error::ChunkSizeOverflow,
        Error::ChunkLineEnding,
        Error::ChunkTerminator,
        Error::ChunkOverhead,
        Error::DecoderFinished,
    ];
    let mut seen = std::collections::HashSet::new();
    for e in all {
        let s = e.to_string();
        assert!(!s.is_empty() && seen.insert(s));
        let _: &dyn std::error::Error = &e;
    }
}

//! Pseudo-random inputs through every entry point of the crate, checking only invariants
//! (no panic, offsets in range, decoded bytes within the buffer). It has no C dependency, so
//! it also runs under Miri (`cargo +nightly miri test -p fuzzdiff`), with fewer iterations.

use fuzzdiff::{
    parse_headers, parse_request, parse_response, ChunkStatus, ChunkedDecoder, Config, Header,
    Status,
};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const ALPHABET: &[u8] = b"GET POST/ HTTP/1.01\r\n\t:;aAfF0xX 2\x00\x7f\x80\xff";

fn random_bytes(r: &mut Rng) -> Vec<u8> {
    let n = r.below(if cfg!(miri) { 40 } else { 200 });
    (0..n).map(|_| ALPHABET[r.below(ALPHABET.len())]).collect()
}

#[test]
fn random_inputs_keep_invariants() {
    let iterations = if cfg!(miri) { 150 } else { 20_000 };
    let mut r = Rng(0x1234_5678_9ABC_DEF1);
    for _ in 0..iterations {
        let data = random_bytes(&mut r);
        let mut headers = vec![Header::EMPTY; r.below(6)];
        let last_len = if r.below(3) == 0 {
            r.below(data.len() + 1)
        } else {
            0
        };
        for cfg in [
            Config::default(),
            Config {
                reject_obs_fold: true,
            },
        ] {
            if let Ok(Status::Complete(q)) = cfg.parse_request(&data, &mut headers, last_len) {
                assert!(q.consumed <= data.len() && q.num_headers <= headers.len());
            }
            if let Ok(Status::Complete(q)) = cfg.parse_response(&data, &mut headers, last_len) {
                assert!(q.consumed <= data.len() && q.num_headers <= headers.len());
            }
            if let Ok(Status::Complete(q)) = cfg.parse_headers(&data, &mut headers, last_len) {
                assert!(q.consumed <= data.len() && q.num_headers <= headers.len());
            }
        }
        // Plain entry points and the chunk decoder, split into random pieces.
        let _ = parse_request(&data, &mut headers, 0);
        let _ = parse_response(&data, &mut headers, 0);
        let _ = parse_headers(&data, &mut headers, 0);
        let mut dec = ChunkedDecoder::new(r.below(2) == 0);
        let mut rest = &data[..];
        while !rest.is_empty() {
            let n = 1 + r.below(rest.len());
            let mut piece = rest[..n].to_vec();
            rest = &rest[n..];
            let d = dec.decode(&mut piece);
            assert!(d.len <= piece.len());
            if let ChunkStatus::Complete { leftover } = d.status {
                assert!(d.len + leftover <= piece.len());
            }
            if d.status != ChunkStatus::Incomplete {
                break;
            }
        }
    }
}

//! Write a libFuzzer seed corpus from corpus/ (control bytes + payload).
//! cargo run -p fuzzdiff-harness --example seeds -- fuzz/corpus/diff

use fuzzdiff_harness::{Case, Op};
use std::{fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("usage: seeds <out-dir>"));
    fs::create_dir_all(&out).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus");
    let mut count = 0;
    for (dir, op) in [
        ("request", Op::Request),
        ("response", Op::Response),
        ("headers", Op::Headers),
        ("chunked", Op::Chunked),
    ] {
        for f in fs::read_dir(root.join(dir)).unwrap() {
            let p = f.unwrap().path();
            let data = fs::read(&p).unwrap();
            let stem = p.file_stem().unwrap().to_string_lossy().to_string();
            let mut variants = vec![Case::new(op, data.clone())];
            let mut full = Case::new(op, data.clone());
            full.last_len = data.len();
            variants.push(full);
            if op == Op::Chunked {
                let mut t = Case::new(op, data.clone()).with_piece_size(3);
                t.consume_trailer = true;
                variants.push(t);
            } else {
                let mut s = Case::new(op, data.clone());
                s.strict = true;
                s.max_headers = 8;
                variants.push(s);
            }
            for (i, v) in variants.iter().enumerate() {
                fs::write(out.join(format!("{dir}-{stem}-{i}")), v.to_seed_bytes()).unwrap();
                count += 1;
            }
        }
    }
    println!("wrote {count} seeds to {}", out.display());
}

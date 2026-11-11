//! Generates the tables in `docs/DIFFERENTIAL_HTTPARSE.md`.
//!
//! ```text
//! cargo run --release -p fuzzdiff-harness --example httparse_table            # print
//! cargo run --release -p fuzzdiff-harness --example httparse_table -- --write # update the doc
//! cargo run --release -p fuzzdiff-harness --example httparse_table -- --check # fail if stale
//! ```
//!
//! Optional `--n <inputs>` changes the sweep size (default 2,000,000; the doc is only
//! comparable at the default).

use fuzzdiff_harness::httparse_report::{load_corpus, render_curated, render_sweep, sweep};
use std::path::PathBuf;
use std::time::Instant;

const DEFAULT_N: u64 = 2_000_000;
const CURATED: (&str, &str) = (
    "<!-- BEGIN GENERATED: curated -->",
    "<!-- END GENERATED: curated -->",
);
const SWEEP: (&str, &str) = (
    "<!-- BEGIN GENERATED: sweep -->",
    "<!-- END GENERATED: sweep -->",
);

fn replace_region(doc: &str, markers: (&str, &str), body: &str) -> String {
    let (Some(a), Some(b)) = (doc.find(markers.0), doc.find(markers.1)) else {
        panic!("markers {markers:?} not found in the document");
    };
    let start = a + markers.0.len();
    format!("{}\n\n{}\n{}", &doc[..start], body.trim_end(), &doc[b..])
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let write = args.iter().any(|a| a == "--write");
    let check = args.iter().any(|a| a == "--check");
    let n = args
        .iter()
        .position(|a| a == "--n")
        .and_then(|i| args.get(i + 1))
        .map_or(DEFAULT_N, |v| v.parse().expect("--n takes a number"));

    let start = Instant::now();
    let corpus = load_corpus();
    let st = sweep(n, &corpus);
    eprintln!("sweep of {n} inputs took {:?}", start.elapsed());
    for e in &st.unclassified_examples {
        eprintln!("unclassified: {:?}", String::from_utf8_lossy(e));
    }
    for (op, e) in &st.early_examples {
        eprintln!(
            "early-reject sample {op:?}: {:?}",
            String::from_utf8_lossy(e)
        );
    }
    let curated = render_curated();
    let sweep_md = render_sweep(&st);

    if !write && !check {
        println!("{curated}\n{sweep_md}");
        return;
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/DIFFERENTIAL_HTTPARSE.md");
    let doc = std::fs::read_to_string(&path).expect("read docs/DIFFERENTIAL_HTTPARSE.md");
    let new = replace_region(&replace_region(&doc, CURATED, &curated), SWEEP, &sweep_md);
    if check {
        if new != doc {
            eprintln!("docs/DIFFERENTIAL_HTTPARSE.md is out of date; run with --write");
            std::process::exit(1);
        }
        eprintln!("docs/DIFFERENTIAL_HTTPARSE.md is up to date");
    } else {
        std::fs::write(&path, new).expect("write doc");
        eprintln!("wrote {}", path.display());
    }
}

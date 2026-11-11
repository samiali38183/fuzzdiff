//! Builds the vendored C parser and the shim.
//!
//! Environment switches (used by CI, not needed locally):
//!   FUZZDIFF_C_SANITIZE=address,undefined   instrument the C code (clang/gcc)
//!   FUZZDIFF_C_SANITIZE_LINK=1              also pass the sanitizer to the linker
//!                                           (for plain `cargo test`; cargo-fuzz links
//!                                           the Rust sanitizer runtime itself)
//!   FUZZDIFF_C_SSE42=1                      compile the C parser with -msse4.2 so its
//!                                           SIMD code path is the one under test

use std::env;

fn main() {
    let c_dir = "../c";
    let mut build = cc::Build::new();
    build
        .file(format!("{c_dir}/picohttpparser.c"))
        .file(format!("{c_dir}/shim.c"))
        .include(c_dir)
        .warnings(false);

    if let Ok(san) = env::var("FUZZDIFF_C_SANITIZE") {
        if !san.is_empty() {
            build.flag(&format!("-fsanitize={san}"));
            build.flag("-fno-omit-frame-pointer");
            build.flag("-fno-sanitize-recover=undefined");
            if env::var("FUZZDIFF_C_SANITIZE_LINK").is_ok_and(|v| v == "1") {
                println!("cargo:rustc-link-arg=-fsanitize={san}");
            }
        }
    }
    if env::var("FUZZDIFF_C_SSE42").is_ok_and(|v| v == "1") {
        build.flag("-msse4.2");
    }

    build.compile("picohttpparser_c");

    for f in ["picohttpparser.c", "picohttpparser.h", "shim.c", "shim.h"] {
        println!("cargo:rerun-if-changed={c_dir}/{f}");
    }
    for v in ["FUZZDIFF_C_SANITIZE", "FUZZDIFF_C_SANITIZE_LINK", "FUZZDIFF_C_SSE42"] {
        println!("cargo:rerun-if-env-changed={v}");
    }
}

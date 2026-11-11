# Common tasks. Fuzzing targets need Linux, nightly Rust, clang and cargo-fuzz.

.PHONY: all test mutants lint cover bench quickfuzz seeds fuzz-diff fuzz-rust fuzz-c asan clean

all: lint test

test:
	cargo test --workspace

mutants:
	cargo test --release -p fuzzdiff-harness --features mutants --test mutants -- --nocapture

lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo clippy --workspace --all-targets --features fuzzdiff-harness/mutants -- -D warnings

cover:
	cargo llvm-cov --workspace --ignore-filename-regex 'harness|build\.rs' --fail-under-lines 90

bench:
	cargo bench -p fuzzdiff-harness --bench throughput

quickfuzz:
	cargo run --release -p fuzzdiff-harness --example quickfuzz -- 60

SECONDS_PER_TARGET ?= 120
FUZZ_ARGS = -max_total_time=$(SECONDS_PER_TARGET) -max_len=4096 -dict=fuzz/http.dict

seeds:
	cargo run -q -p fuzzdiff-harness --example seeds -- fuzz/corpus/diff
	mkdir -p fuzz/corpus/rust_only fuzz/corpus/c_only
	cp fuzz/corpus/diff/* fuzz/corpus/rust_only/
	cp fuzz/corpus/diff/* fuzz/corpus/c_only/

fuzz-diff: seeds
	FUZZDIFF_C_SANITIZE=address,fuzzer-no-link CC=clang \
	cargo +nightly fuzz run diff fuzz/corpus/diff -- $(FUZZ_ARGS)

fuzz-rust: seeds
	cargo +nightly fuzz run rust_only fuzz/corpus/rust_only -- $(FUZZ_ARGS)

fuzz-c: seeds
	FUZZDIFF_C_SANITIZE=address,undefined,fuzzer-no-link CC=clang \
	RUSTFLAGS="-C linker=clang -C link-arg=-fsanitize=undefined" \
	cargo +nightly fuzz run c_only fuzz/corpus/c_only -- $(FUZZ_ARGS)

# Plain cargo test with the C code under ASan and UBSan (Linux, gcc or clang).
asan:
	FUZZDIFF_C_SANITIZE=address,undefined FUZZDIFF_C_SANITIZE_LINK=1 cargo test --workspace --release

clean:
	cargo clean

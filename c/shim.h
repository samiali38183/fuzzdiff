/* Thin shim around picohttpparser for the differential harness.
 *
 * Differences from calling the library directly:
 *  - results are offsets into the input, not pointers, so they can be compared
 *    with the Rust parser output;
 *  - the input is copied into a heap block of exactly `len` bytes and the header
 *    array is a heap block of exactly `max_headers` entries, so AddressSanitizer
 *    flags any read or write past either one;
 *  - the chunked decoder runs a whole multi-call session in one shim call.
 *
 * Callers must not pass last_len > len: the C library reads out of bounds then.
 */
#ifndef FUZZDIFF_SHIM_H
#define FUZZDIFF_SHIM_H

#include <stddef.h>
#include <stdint.h>

typedef struct {
    int64_t name_off; /* -1 for a continuation line (name == NULL) */
    size_t name_len;
    size_t value_off;
    size_t value_len;
} fd_header;

typedef struct {
    size_t method_off, method_len;
    size_t path_off, path_len;
    size_t reason_off, reason_len;
    int32_t minor_version;
    int32_t status;
    size_t num_headers;
} fd_head;

/* All return the C result: bytes consumed, -2 partial, -1 error.
 * `out_headers` must have room for `max_headers` entries. */
int64_t fd_parse_request(const uint8_t *data, size_t len, size_t last_len, fd_header *out_headers, size_t max_headers,
                         fd_head *out);
int64_t fd_parse_response(const uint8_t *data, size_t len, size_t last_len, fd_header *out_headers, size_t max_headers,
                          fd_head *out);
int64_t fd_parse_headers(const uint8_t *data, size_t len, size_t last_len, fd_header *out_headers, size_t max_headers,
                         fd_head *out);

typedef struct {
    int64_t ret;     /* phr_decode_chunked result */
    size_t newlen;   /* decoded length reported for this call */
    int32_t in_data; /* phr_decode_chunked_is_in_data after the call */
} fd_chunk_step;

typedef struct {
    size_t steps;    /* number of calls made (<= npieces; stops at the first result other than -2) */
    size_t out_len;  /* decoded bytes written to `out` */
    size_t left_len; /* undecoded bytes after the end of the body, written to `left` */
} fd_chunk_result;

/* Feed `data` to one decoder in pieces of the given lengths (they should sum to
 * `len`). `steps` has room for `npieces`, `out` and `left` for `len` bytes. */
void fd_chunked(const uint8_t *data, size_t len, const size_t *piece_lens, size_t npieces, int consume_trailer,
                fd_chunk_step *steps, uint8_t *out, uint8_t *left, fd_chunk_result *res);

/* Benchmark entry points: no allocation, stack header array of 64. */
int64_t fd_bench_request(const uint8_t *data, size_t len);
int64_t fd_bench_response(const uint8_t *data, size_t len);
int64_t fd_bench_chunked(const uint8_t *data, size_t len, uint8_t *scratch);

#endif

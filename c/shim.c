#include "shim.h"

#include <stdlib.h>
#include <string.h>

#include "picohttpparser.h"

/* Heap copy of exactly `len` bytes (at least one byte allocated so the pointer is valid). */
static char *dup_exact(const uint8_t *data, size_t len)
{
    char *p = malloc(len ? len : 1);
    if (p == NULL)
        abort();
    if (len)
        memcpy(p, data, len);
    return p;
}

static struct phr_header *alloc_headers(size_t max_headers)
{
    struct phr_header *h = malloc((max_headers ? max_headers : 1) * sizeof(*h));
    if (h == NULL)
        abort();
    return h;
}

static void export_headers(const char *base, const struct phr_header *h, size_t n, fd_header *out)
{
    size_t i;
    for (i = 0; i < n; i++) {
        out[i].name_off = h[i].name == NULL ? -1 : (int64_t)(h[i].name - base);
        out[i].name_len = h[i].name_len;
        out[i].value_off = (size_t)(h[i].value - base);
        out[i].value_len = h[i].value_len;
    }
}

int64_t fd_parse_request(const uint8_t *data, size_t len, size_t last_len, fd_header *out_headers, size_t max_headers,
                         fd_head *out)
{
    char *buf = dup_exact(data, len);
    struct phr_header *h = alloc_headers(max_headers);
    const char *method, *path;
    size_t method_len, path_len, n = max_headers;
    int minor;
    int r;
    memset(out, 0, sizeof(*out));
    r = phr_parse_request(buf, len, &method, &method_len, &path, &path_len, &minor, h, &n, last_len);
    if (r >= 0) {
        out->method_off = (size_t)(method - buf);
        out->method_len = method_len;
        out->path_off = (size_t)(path - buf);
        out->path_len = path_len;
        out->minor_version = minor;
        out->num_headers = n;
        export_headers(buf, h, n, out_headers);
    }
    free(h);
    free(buf);
    return r;
}

int64_t fd_parse_response(const uint8_t *data, size_t len, size_t last_len, fd_header *out_headers, size_t max_headers,
                          fd_head *out)
{
    char *buf = dup_exact(data, len);
    struct phr_header *h = alloc_headers(max_headers);
    const char *msg;
    size_t msg_len, n = max_headers;
    int minor, status;
    int r;
    memset(out, 0, sizeof(*out));
    r = phr_parse_response(buf, len, &minor, &status, &msg, &msg_len, h, &n, last_len);
    if (r >= 0) {
        out->minor_version = minor;
        out->status = status;
        out->reason_off = (size_t)(msg - buf);
        out->reason_len = msg_len;
        out->num_headers = n;
        export_headers(buf, h, n, out_headers);
    }
    free(h);
    free(buf);
    return r;
}

int64_t fd_parse_headers(const uint8_t *data, size_t len, size_t last_len, fd_header *out_headers, size_t max_headers,
                         fd_head *out)
{
    char *buf = dup_exact(data, len);
    struct phr_header *h = alloc_headers(max_headers);
    size_t n = max_headers;
    int r;
    memset(out, 0, sizeof(*out));
    r = phr_parse_headers(buf, len, h, &n, last_len);
    if (r >= 0) {
        out->num_headers = n;
        export_headers(buf, h, n, out_headers);
    }
    free(h);
    free(buf);
    return r;
}

void fd_chunked(const uint8_t *data, size_t len, const size_t *piece_lens, size_t npieces, int consume_trailer,
                fd_chunk_step *steps, uint8_t *out, uint8_t *left, fd_chunk_result *res)
{
    struct phr_chunked_decoder dec;
    size_t off = 0, i;
    memset(&dec, 0, sizeof(dec));
    dec.consume_trailer = (char)consume_trailer;
    memset(res, 0, sizeof(*res));
    for (i = 0; i < npieces; i++) {
        size_t n = piece_lens[i];
        size_t bufsz;
        char *buf;
        ssize_t r;
        if (n > len - off)
            n = len - off;
        buf = dup_exact(data + off, n);
        bufsz = n;
        r = phr_decode_chunked(&dec, buf, &bufsz);
        steps[i].ret = (int64_t)r;
        steps[i].newlen = bufsz;
        steps[i].in_data = phr_decode_chunked_is_in_data(&dec);
        res->steps = i + 1;
        if (bufsz)
            memcpy(out + res->out_len, buf, bufsz);
        res->out_len += bufsz;
        if (r > 0) {
            memcpy(left, buf + bufsz, (size_t)r);
            res->left_len = (size_t)r;
        }
        free(buf);
        off += n;
        if (r != -2)
            break;
    }
}

int64_t fd_bench_request(const uint8_t *data, size_t len)
{
    struct phr_header h[64];
    const char *method, *path;
    size_t method_len, path_len, n = 64;
    int minor;
    int r = phr_parse_request((const char *)data, len, &method, &method_len, &path, &path_len, &minor, h, &n, 0);
    if (r < 0)
        return r;
    return (int64_t)r + (int64_t)n + (int64_t)method_len + (int64_t)path_len + minor;
}

int64_t fd_bench_response(const uint8_t *data, size_t len)
{
    struct phr_header h[64];
    const char *msg;
    size_t msg_len, n = 64;
    int minor, status;
    int r = phr_parse_response((const char *)data, len, &minor, &status, &msg, &msg_len, h, &n, 0);
    if (r < 0)
        return r;
    return (int64_t)r + (int64_t)n + (int64_t)msg_len + status + minor;
}

int64_t fd_bench_chunked(const uint8_t *data, size_t len, uint8_t *scratch)
{
    struct phr_chunked_decoder dec;
    size_t bufsz = len;
    ssize_t r;
    memset(&dec, 0, sizeof(dec));
    memcpy(scratch, data, len);
    r = phr_decode_chunked(&dec, (char *)scratch, &bufsz);
    return (int64_t)r + (int64_t)bufsz;
}

"""Write the hand-written corpus cases (edge cases and overflow bait) into corpus/.

Run once; the generated files are committed so the corpus is reviewable as data.
"""
import pathlib

root = pathlib.Path(__file__).resolve().parent.parent / "corpus"

REQ = {
    "bare-lf-endings": b"GET / HTTP/1.1\nHost: a\n\n",
    "mixed-endings": b"GET / HTTP/1.1\r\nHost: a\nX: b\r\n\n",
    "leading-crlf": b"\r\nGET / HTTP/1.1\r\n\r\n",
    "leading-lf": b"\nGET / HTTP/1.1\r\n\r\n",
    "only-crlf": b"\r\n",
    "only-cr": b"\r",
    "empty": b"",
    "single-nul": b"\x00",
    "fold-first-header": b"GET / HTTP/1.1\r\n folded: x\r\nHost: a\r\n\r\n",
    "fold-after-header": b"GET / HTTP/1.1\r\nHost: a\r\n b\r\n\tc\r\nX: y\r\n\r\n",
    "fold-whitespace-only": b"GET / HTTP/1.1\r\nHost: a\r\n \t \r\n\r\n",
    "fold-many": b"GET / HTTP/1.1\r\nA: 1\r\n 2\r\n 3\r\n 4\r\n 5\r\nB: 6\r\n\r\n",
    "duplicate-host": b"GET / HTTP/1.1\r\nHost: a\r\nHost: b\r\n\r\n",
    "duplicate-content-length": b"POST / HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\n",
    "cl-te-smuggling-shape": b"POST / HTTP/1.1\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
    "header-value-17": b"GET / HTTP/1.1\r\nX: " + b"a" * 17 + b"\r\n\r\n",
    "header-value-33": b"GET / HTTP/1.1\r\nX: " + b"b" * 33 + b"\r\n\r\n",
    "header-value-trailing-ws-long": b"GET / HTTP/1.1\r\nX: " + b"c" * 20 + b" \t \r\n\r\n",
    "no-space-after-colon": b"GET / HTTP/1.1\r\nHost:a\r\n\r\n",
    "tab-after-colon": b"GET / HTTP/1.1\r\nHost:\t\ta\r\n\r\n",
    "empty-value": b"GET / HTTP/1.1\r\nHost:\r\n\r\n",
    "colon-at-eof": b"GET / HTTP/1.1\r\nHost:",
    "name-no-colon": b"GET / HTTP/1.1\r\nHost\r\n\r\n",
    "nul-in-path": b"GET /a\x00b HTTP/1.1\r\n\r\n",
    "nul-in-value-end": b"GET / HTTP/1.1\r\nX: a\x00",
    "high-bit-everywhere": b"G\xe9T / HTTP/1.1\r\n\r\n",
    "high-bit-path": b"GET /\xff\xfe\xfd HTTP/1.1\r\nX: \xc3\x28\r\n\r\n",
    "del-in-value": b"GET / HTTP/1.1\r\nX: a\x7fb\r\n\r\n",
    "cr-in-value": b"GET / HTTP/1.1\r\nX: a\rb\r\n\r\n",
    "cr-then-eof-in-value": b"GET / HTTP/1.1\r\nX: a\r",
    "version-1.9": b"GET / HTTP/1.9\r\n\r\n",
    "version-1.10": b"GET / HTTP/1.10\r\n\r\n",
    "version-2.0": b"GET / HTTP/2.0\r\n\r\n",
    "version-lowercase": b"GET / http/1.1\r\n\r\n",
    "version-eight-bytes-exact": b"GET / HTTP/1.1",
    "version-nine-bytes-garbage": b"GET / HTTP/1.1X",
    "version-junk-short": b"GET / HXXXXXX",
    "path-with-spaces-many": b"GET      /      HTTP/1.1\r\n\r\n",
    "method-only-spaces": b"    ",
    "method-then-eof": b"GET",
    "absolute-uri": b"GET http://example.com/a?b=c#d HTTP/1.1\r\nHost: example.com\r\n\r\n",
    "options-star": b"OPTIONS * HTTP/1.1\r\n\r\n",
    "connect-authority": b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n",
    "token-punctuation": b"M!#$%&'*+-.^_`|~ / HTTP/1.1\r\n!#$%&'*+-.^_`|~: v\r\n\r\n",
    "token-brace": b"GET / HTTP/1.1\r\n{x: v\r\n\r\n",
    "pipelined-second-request": b"GET /a HTTP/1.1\r\n\r\nGET /b HTTP/1.1\r\n\r\n",
    "body-after-head": b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc",
    "lf-cr-lf": b"GET / HTTP/1.1\n\r\n",
    "many-headers-40": b"GET / HTTP/1.1\r\n" + b"".join(b"H%d: v%d\r\n" % (i, i) for i in range(40)) + b"\r\n",
    "many-headers-100": b"GET / HTTP/1.1\r\n" + b"".join(b"H%d: v%d\r\n" % (i, i) for i in range(100)) + b"\r\n",
    "long-path-5000": b"GET /" + b"p" * 5000 + b" HTTP/1.1\r\n\r\n",
    "long-header-name-300": b"GET / HTTP/1.1\r\n" + b"N" * 300 + b": v\r\n\r\n",
    "browser-like": (
        b"GET /search?q=memory+safety HTTP/1.1\r\nHost: www.example.com\r\nUser-Agent: Mozilla/5.0 (X11; Linux x86_64) "
        b"AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36\r\nAccept: text/html,application/xhtml+xml,"
        b"application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8\r\nAccept-Language: en-US,en;q=0.9\r\nAccept-Encoding: gzip, "
        b"deflate, br\r\nConnection: keep-alive\r\nCookie: session=0123456789abcdef0123456789abcdef; theme=dark\r\n"
        b"Upgrade-Insecure-Requests: 1\r\nCache-Control: max-age=0\r\n\r\n"
    ),
}

RESP = {
    "status-100": b"HTTP/1.1 100 Continue\r\n\r\n",
    "status-000": b"HTTP/1.1 000 X\r\n\r\n",
    "status-199": b"HTTP/1.1 199 X\r\n\r\n",
    "status-4digits": b"HTTP/1.1 2000 X\r\n\r\n",
    "status-alpha": b"HTTP/1.1 2a0 X\r\n\r\n",
    "reason-empty-with-space": b"HTTP/1.1 200 \r\n\r\n",
    "reason-all-spaces": b"HTTP/1.1 200      \r\n\r\n",
    "reason-tab": b"HTTP/1.1 200\tOK\r\n\r\n",
    "reason-high-bit": b"HTTP/1.1 200 \xe9\xe8\r\n\r\n",
    "status-exact-four-bytes": b"HTTP/1.1 200\n",
    "status-three-digits-eof": b"HTTP/1.1 200",
    "no-space-after-version": b"HTTP/1.1200 OK\r\n\r\n",
    "bare-lf": b"HTTP/1.1 200 OK\nContent-Length: 0\n\n",
    "chunked-response-head": b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n",
    "fold-in-response": b"HTTP/1.1 200 OK\r\nX: a\r\n b\r\n\r\n",
    "long-reason": b"HTTP/1.1 200 " + b"R" * 400 + b"\r\n\r\n",
    "value-17": b"HTTP/1.1 200 OK\r\nX: " + b"v" * 17 + b"\r\n\r\n",
}

HDRS = {
    "empty-block": b"\r\n",
    "empty-block-lf": b"\n",
    "fold-first": b" a\r\n\r\n",
    "single-colon-line": b":\r\n\r\n",
    "many-trailers": b"".join(b"T%d: %d\r\n" % (i, i) for i in range(20)) + b"\r\n",
    "value-long-17": b"X-Checksum: " + b"0123456789abcdef0" + b"\r\n\r\n",
    "trailer-fold": b"A: 1\r\n  2\r\n\r\n",
}

CHUNKED = {
    "size-max-64bit": b"ffffffffffffffff\r\nabc",
    "size-17-digits": b"10000000000000000\r\nabc",
    "size-leading-zeros-19": b"0000000000000000005\r\nhello\r\n0\r\n",
    "size-leading-zeros-16": b"0000000000000005\r\nhello\r\n0\r\n",
    "size-7fff": b"7fffffffffffffff\r\nabc",
    "size-uppercase": b"A\r\n0123456789\r\n0\r\n\r\n",
    "size-empty": b"\r\nabc",
    "size-negative-looking": b"-1\r\nabc",
    "size-0x-prefix": b"0x5\r\nhello\r\n0\r\n",
    "size-space-tab-ext": b"5 \t;x=1\r\nhello\r\n0\r\n\r\n",
    "size-then-eof": b"5",
    "ext-long": b"5;" + b"e" * 3000 + b"\r\nhello\r\n0\r\n\r\n",
    "ext-bare-lf": b"5;x\nhello\r\n0\r\n",
    "ext-cr-then-x": b"5;x\rXhello\r\n0\r\n",
    "data-short": b"a\r\nabc",
    "data-missing-crlf": b"3\r\nabcXY0\r\n",
    "data-cr-only": b"3\r\nabc\r0\r\n",
    "data-lf-only": b"3\r\nabc\n0\r\n",
    "zero-chunk-only": b"0\r\n",
    "zero-chunk-with-ext": b"0;x=1\r\n\r\n",
    "zero-chunk-trailers": b"0\r\nX-A: 1\r\nX-B: 2\r\n\r\n",
    "zero-chunk-trailer-bare-lf": b"0\r\nX-A: 1\nX-B: 2\n\n",
    "zero-chunk-trailer-cr-run": b"0\r\n\r\r\r\n",
    "zero-chunk-trailer-no-end": b"0\r\nX-A: 1\r\nX-B: 2",
    "zero-chunk-trailer-cr-in-line": b"0\r\nA\rB\r\n\r\n",
    "after-end-pipelined": b"5\r\nabcde\r\n0\r\n\r\nGET / HTTP/1.1\r\n\r\n",
    "after-end-no-trailer-mode": b"5\r\nabcde\r\n0\r\nGET / HTTP/1.1\r\n\r\n",
    "binary-body": b"8\r\n\x00\x01\xff\xfe\r\n\r\n\r\n0\r\n\r\n",
    "many-tiny-chunks": b"".join(b"1\r\nx\r\n" for _ in range(300)) + b"0\r\n\r\n",
    "only-crlf": b"\r\n",
    "empty": b"",
    "nul-size": b"\x00\r\n",
    "large-chunk": b"1000\r\n" + b"z" * 4096 + b"\r\n0\r\n\r\n",
}

for kind, table in (("request", REQ), ("response", RESP), ("headers", HDRS), ("chunked", CHUNKED)):
    d = root / kind
    d.mkdir(parents=True, exist_ok=True)
    for name, data in table.items():
        (d / f"hand-{name}.bin").write_bytes(data)
    print(kind, len(table))

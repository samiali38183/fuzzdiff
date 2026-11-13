# Three-way differential: picohttpparser, fuzzdiff, httparse

fuzzdiff is compared against picohttpparser, its C original, and the tests require them to
agree (see [DEVIATIONS](DEVIATIONS.md)). That says nothing about how either behaves against
a second, independent parser. This page adds one: [httparse](https://github.com/seanmonstar/httparse)
1.10.1 (MIT or Apache-2.0, pinned exactly in `harness/Cargo.toml`), the HTTP/1.x parser
hyper builds on. The result is a map of where "matches picohttpparser" and "matches a
widely used Rust parser" are different statements. It is not a list of bugs: each difference
below is a leniency choice that one or both libraries made, and each is pinned by a test.

Scope and limits:

* Head parsing (request line or status line, header block) and the first chunk-size line.
  httparse offers no chunked decoder, only `parse_chunk_size`, so chunk-size lines are
  compared as a single line with a well-formed body after it, and chunked bodies are not
  swept.
* httparse runs with its default configuration. It has options (for example
  `allow_obsolete_multiline_headers_in_responses`) that make it more lenient; the table
  describes the defaults only.
* fuzzdiff runs with its default `Config`, which is byte-for-byte equivalent to the C
  library. With `Config { reject_obs_fold: true }` (deviation D1) it rejects folds too.
* Both sides were given 64 header slots.
* This compares two parsers on one input. It does not model a proxy in front of a server,
  and nothing here is a working attack against any system.
* RFC text was read from RFC 9112 and RFC 9110 (June 2022) and RFC 3986. Section numbers
  below are from those documents. httparse behaviour was read from its 1.10.1 source and
  confirmed by running it.

## How the comparison runs

`harness/src/httparse_oracle.rs` runs one input through all three parsers and reduces each
result to complete, partial or reject, plus the parsed fields when all three complete. The
outcome pattern is three letters in the order picohttpparser, fuzzdiff, httparse, for example
`CCR`: the two C-equivalent parsers complete, httparse rejects. A trailing `!` means all
three completed but returned different fields.

Two input sets:

1. A curated list of minimal examples, one or more per class
   (`harness/src/httparse_cases.rs`). These are the "documented deviations".
2. A deterministic sweep (seed = input index + 1) of 2,000,000 inputs: half start from a
   `corpus/` file, half from the well-formed generators, two thirds are then mutated
   (byte flips, inserts, deletes, truncation, duplicated ranges, splices of CR, LF, NUL,
   SP, HTAB, colon, `HTTP/1.`, 0x7f, 0xff).

Every disagreeing input in the sweep is attributed to the smallest set of features whose
rewrite-and-remove makes the three parsers agree (so "high bytes" are not blamed just because
they are present in most inputs). An input no feature explains is counted as unexplained.
The CI test fails on any unexplained disagreement, on any new feature appearing in the
attribution, on any new `(entry point, pattern)` combination, and on any difference between
picohttpparser and fuzzdiff.

The tables are generated. Regenerate with

```
cargo run --release -p fuzzdiff-harness --example httparse_table -- --write
```

(about 20 s on the machine used for the numbers below; `--check` fails when the file is
stale, and the curated table is also checked by `cargo test`).

## Curated classes

<!-- BEGIN GENERATED: curated -->

| Class | Example | picohttpparser | fuzzdiff | httparse | Pattern | Stricter |
|---|---|---|---|---|---|---|
| Accepted by all three (context) | `GET / HTTP/1.1\r\nHost: a\r\n\r\n` (baseline request) | complete | complete | complete | CCC | same |
| Accepted by all three (context) | `POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n` (Content-Length and Transfer-Encoding both present) | complete | complete | complete | CCC | same |
| Accepted by all three (context) | `POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 4\r\nContent-Length: 5\r\n\r\n` (duplicate Content-Length, different values) | complete | complete | complete | CCC | same |
| Accepted by all three (context) | `POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: xchunked\r\n\r\n` (Transfer-Encoding: xchunked) | complete | complete | complete | CCC | same |
| Accepted by all three (context) | `GET http://a/ HTTP/1.1\r\nHost: a\r\n\r\n` (absolute-form target) | complete | complete | complete | CCC | same |
| Accepted by all three (context) | `OPTIONS * HTTP/1.1\r\nHost: a\r\n\r\n` (asterisk-form target) | complete | complete | complete | CCC | same |
| Accepted by all three (context) | `get / HTTP/1.1\r\n\r\n` (lower-case method) | complete | complete | complete | CCC | same |
| Accepted by all three (context) | `G@T / HTTP/1.1\r\n\r\n` (non-token method) | reject | reject | reject | RRR | same |
| Accepted by all three (context) | `GET / HTTP/1.1 \r\n\r\n` (trailing SP after the version) | reject | reject | reject | RRR | same |
| Accepted by all three (context) | `GET / HTTP/1.1\r\nA: b\tc\r\n\r\n` (HTAB inside a field value) | complete | complete | complete | CCC | same |
| Accepted by all three (context) | `GET / HTTP/1.1\r\n: x\r\n\r\n` (empty field name) | reject | reject | reject | RRR | same |
| Accepted by all three (context) | `GET / HTTP/1.1\r\nfoo\r\n\r\n` (header line without a colon) | reject | reject | reject | RRR | same |
| Accepted by all three (context) | `HTTP/1.1 200\r\n\r\n` (response with no reason phrase) | complete | complete | complete | CCC | same |
| Obsolete line folding | `GET / HTTP/1.1\r\nA: b\r\n c\r\n\r\n` (request, SP continuation) | complete | complete | reject | CCR | httparse |
| Obsolete line folding | `GET / HTTP/1.1\r\nA: b\r\n\tc\r\n\r\n` (request, HTAB continuation) | complete | complete | reject | CCR | httparse |
| Obsolete line folding | `A: b\r\n c\r\n\r\n` (header block, SP continuation) | complete | complete | reject | CCR | httparse |
| Obsolete line folding | `HTTP/1.1 200 OK\r\nA: b\r\n c\r\n\r\n` (response, SP continuation) | complete | complete | reject | CCR | httparse |
| Whitespace before the first header line | `GET / HTTP/1.1\r\n A: b\r\n\r\n` (request, SP before the first header) | reject | reject | reject | RRR | same |
| Whitespace before the first header line | ` A: b\r\n\r\n` (header block, SP before the first header) | reject | reject | reject | RRR | same |
| Number of SP between start-line elements | `GET  / HTTP/1.1\r\n\r\n` (two SP after the method) | complete | complete | reject | CCR | httparse |
| Number of SP between start-line elements | `GET /  HTTP/1.1\r\n\r\n` (two SP before the version) | complete | complete | reject | CCR | httparse |
| Number of SP between start-line elements | `HTTP/1.1 200  OK\r\n\r\n` (two SP after the status code) | complete | complete | complete | CCC! | same verdict, fields differ |
| Other whitespace as a start-line separator | `GET\t/ HTTP/1.1\r\n\r\n` (HTAB after the method) | reject | reject | reject | RRR | same |
| Other whitespace as a start-line separator | `GET /\tHTTP/1.1\r\n\r\n` (HTAB before the version) | reject | reject | reject | RRR | same |
| HTTP version digits | `GET / HTTP/1.2\r\n\r\n` (HTTP/1.2 request) | complete | complete | reject | CCR | httparse |
| HTTP version digits | `GET / HTTP/1.9\r\n\r\n` (HTTP/1.9 request) | complete | complete | reject | CCR | httparse |
| HTTP version digits | `HTTP/1.2 200 OK\r\n\r\n` (HTTP/1.2 response) | complete | complete | reject | CCR | httparse |
| HTTP version digits | `GET / HTTP/2.0\r\n\r\n` (HTTP/2.0 request) | reject | reject | reject | RRR | same |
| HTTP version digits | `GET / HTTP/0.9\r\n\r\n` (HTTP/0.9 request) | reject | reject | reject | RRR | same |
| HTTP version digits | `GET / HTTP/1.10\r\n\r\n` (HTTP/1.10 request) | reject | reject | reject | RRR | same |
| HTTP version digits | `GET / http/1.1\r\n\r\n` (lower-case http/1.1) | reject | reject | reject | RRR | same |
| Octets 0x80-0xFF in the request-target | `GET /\x80 HTTP/1.1\r\n\r\n` (0x80 in the target) | complete | complete | reject | CCR | httparse |
| Octets 0x80-0xFF in the request-target | `GET /\xff HTTP/1.1\r\n\r\n` (0xFF in the target) | complete | complete | reject | CCR | httparse |
| Octets 0x80-0xFF in a field value | `GET / HTTP/1.1\r\nA: \x80\r\n\r\n` (0x80 in a field value) | complete | complete | complete | CCC | same |
| Octets 0x80-0xFF in a field value | `HTTP/1.1 200 OK\r\nA: \xff\r\n\r\n` (0xFF in a response field value) | complete | complete | complete | CCC | same |
| Octets 0x80-0xFF in a field name | `GET / HTTP/1.1\r\n\x80: x\r\n\r\n` (0x80 in a field name) | reject | reject | reject | RRR | same |
| Field-name characters outside tchar | `GET / HTTP/1.1\r\nA@B: x\r\n\r\n` (@ in a field name) | reject | reject | reject | RRR | same |
| Field-name characters outside tchar | `GET / HTTP/1.1\r\nA(B: x\r\n\r\n` (( in a field name) | reject | reject | reject | RRR | same |
| Field-name characters outside tchar | `GET / HTTP/1.1\r\nA B: x\r\n\r\n` (SP inside a field name) | reject | reject | reject | RRR | same |
| Field-name characters outside tchar | `GET / HTTP/1.1\r\nA_B~: x\r\n\r\n` (underscore and tilde in a field name (valid tchar)) | complete | complete | complete | CCC | same |
| Whitespace between field name and colon | `GET / HTTP/1.1\r\nA : b\r\n\r\n` (SP before the colon, request) | reject | reject | reject | RRR | same |
| Whitespace between field name and colon | `HTTP/1.1 200 OK\r\nA : b\r\n\r\n` (SP before the colon, response) | reject | reject | reject | RRR | same |
| Whitespace between field name and colon | `GET / HTTP/1.1\r\nA\t: b\r\n\r\n` (HTAB before the colon) | reject | reject | reject | RRR | same |
| Bare LF as line terminator | `GET / HTTP/1.1\nHost: a\n\n` (LF-only line endings) | complete | complete | complete | CCC | same |
| Bare LF as line terminator | `GET / HTTP/1.1\nHost: a\r\n\r\n` (LF after the start-line only) | complete | complete | complete | CCC | same |
| Bare CR | `GET / HTTP/1.1\r\r\nHost: a\r\n\r\n` (bare CR after the start-line) | reject | reject | reject | RRR | same |
| Bare CR | `GET / HTTP/1.1\r\nA: b\rc\r\n\r\n` (bare CR inside a field value) | reject | reject | reject | RRR | same |
| NUL and other control octets | `GET / HTTP/1.1\r\nA: b\x00c\r\n\r\n` (NUL in a field value) | reject | reject | reject | RRR | same |
| NUL and other control octets | `GET /\x00 HTTP/1.1\r\n\r\n` (NUL in the target) | reject | reject | reject | RRR | same |
| NUL and other control octets | `GET / HTTP/1.1\r\nA: b\x7fc\r\n\r\n` (DEL in a field value) | reject | reject | reject | RRR | same |
| NUL and other control octets | `GET / HTTP/1.1\r\nA: b\x01c\r\n\r\n` (0x01 in a field value) | reject | reject | reject | RRR | same |
| NUL and other control octets | `GET /\x01 HTTP/1.1\r\n\r\n` (0x01 in the target) | reject | reject | reject | RRR | same |
| Empty lines before the start-line | `\r\nGET / HTTP/1.1\r\n\r\n` (one empty line before the request) | complete | complete | complete | CCC | same |
| Empty lines before the start-line | `\r\n\r\nGET / HTTP/1.1\r\n\r\n` (two empty lines before the request) | reject | reject | complete | RRC | picohttpparser |
| Status-code range | `HTTP/1.1 600 X\r\n\r\n` (status 600) | reject | reject | complete | RRC | picohttpparser |
| Status-code range | `HTTP/1.1 099 X\r\n\r\n` (status 099) | reject | reject | complete | RRC | picohttpparser |
| Status-code range | `HTTP/1.1 999 X\r\n\r\n` (status 999) | reject | reject | complete | RRC | picohttpparser |
| Status-code range | `HTTP/1.1 20 X\r\n\r\n` (two-digit status) | reject | reject | reject | RRR | same |
| Reason-phrase content | `HTTP/1.1 200 OK\xff\r\n\r\n` (0xFF in the reason phrase) | complete | complete | complete | CCC! | same verdict, fields differ |
| Reason-phrase content | `HTTP/1.1 200 \tOK\r\n\r\n` (HTAB in the reason phrase) | complete | complete | complete | CCC | same |
| Reason-phrase content | `HTTP/1.1 200 O\x01K\r\n\r\n` (0x01 in the reason phrase) | reject | reject | reject | RRR | same |
| Partial input: when a parser gives up | `GET / HTTP/1.` (version cut after 'HTTP/1.') | partial | partial | partial | PPP | same |
| Partial input: when a parser gives up | `GET / HX\r\n\r\nxy` (invalid version prefix, 8 bytes available) | partial | partial | reject | PPR | httparse (rejects earlier) |
| Partial input: when a parser gives up | `GE\x01` (invalid method byte after a full method) | reject | reject | reject | RRR | same |
| Partial input: when a parser gives up | `GET / HTTP/1.1\r\nA: b\r\n c` (obs-fold line cut before its end) | partial | partial | reject | PPR | httparse (rejects earlier) |
| Chunk extensions | `5\r\nhello\r\n0\r\n\r\n` (no extension (baseline)) | complete | complete | complete | CCC | same |
| Chunk extensions | `5;a=b\r\nhello\r\n0\r\n\r\n` (name=value extension) | complete | complete | complete | CCC | same |
| Chunk extensions | `5;a\r\nhello\r\n0\r\n\r\n` (name-only extension) | complete | complete | complete | CCC | same |
| Chunk extensions | `5;\r\nhello\r\n0\r\n\r\n` (empty extension (';' then CRLF)) | complete | complete | complete | CCC | same |
| Chunk extensions | `5;a=\"b c\"\r\nhello\r\n0\r\n\r\n` (quoted-string value) | complete | complete | complete | CCC | same |
| Chunk extensions | `5 ; a = b\r\nhello\r\n0\r\n\r\n` (BWS around ';' and '=') | complete | complete | complete | CCC | same |
| Chunk extensions | `5;a=\xff\r\nhello\r\n0\r\n\r\n` (0xFF in an extension) | complete | complete | complete | CCC | same |
| Chunk extensions | `5;a=\x00\r\nhello\r\n0\r\n\r\n` (NUL in an extension) | complete | complete | complete | CCC | same |
| Chunk extensions | `5;a=b\rc\r\nhello\r\n0\r\n\r\n` (bare CR in an extension) | reject | reject | reject | RRR | same |
| Chunk extensions | `5;a=b\nhello\r\n0\r\n\r\n` (bare LF inside an extension) | reject | reject | complete | RRC | picohttpparser |
| Chunk-size line shape | `5 \r\nhello\r\n0\r\n\r\n` (SP after the size) | complete | complete | complete | CCC | same |
| Chunk-size line shape | `5\t\r\nhello\r\n0\r\n\r\n` (HTAB after the size) | complete | complete | complete | CCC | same |
| Chunk-size line shape | `5\nhello\r\n0\r\n\r\n` (bare LF after the size) | reject | reject | reject | RRR | same |
| Chunk-size line shape | `A\r\n0123456789\r\n0\r\n\r\n` (upper-case hex digit) | complete | complete | complete | CCC | same |
| Chunk-size line shape | `0005\r\nhello\r\n0\r\n\r\n` (leading zeros) | complete | complete | complete | CCC | same |
| Chunk-size line shape | `0x5\r\nhello\r\n0\r\n\r\n` (0x prefix) | reject | reject | reject | RRR | same |
| Chunk-size line shape | `+5\r\nhello\r\n0\r\n\r\n` (plus sign) | reject | reject | reject | RRR | same |
| Chunk-size line shape | `\r\nhello\r\n0\r\n\r\n` (empty size) | reject | reject | complete | RRC | picohttpparser |
| Chunk-size line shape | `5g\r\nhello\r\n0\r\n\r\n` (non-hex byte after the size) | reject | reject | reject | RRR | same |
| Chunk-size line shape | `10000000000000000\r\nhello` (17 hex digits (overflow of u64)) | reject | reject | reject | RRR | same |
| Chunk-size line shape | `000000000000000005\r\nhello\r\n0\r\n\r\n` (18 hex digits with leading zeros) | reject | reject | reject | RRR | same |
<!-- END GENERATED: curated -->

"Stricter" names the parser that rejects, or rejects earlier. Where it says `same`, the three
parsers behave alike on that input; those rows are there so the table also shows what is not
a difference (for example, all three accept 0x80 in a field value and all three reject
whitespace before the colon).

## Sweep results

<!-- BEGIN GENERATED: sweep -->

Inputs: 2000000 (999999 request, 500001 response, 500000 header block). All three parsers did the same thing on 1390722 (69.54%); they disagreed on 609278.

picohttpparser and fuzzdiff differed on 0 inputs (a non-zero number would be a bug and fail the tests). Disagreements explained by no known feature: 0. Disagreements that are only httparse rejecting an invalid prefix earlier: 36905.

Disagreements by entry point and outcome pattern (picohttpparser, fuzzdiff, httparse; `C` complete, `P` partial, `R` reject, `!` all complete but fields differ):

| Entry point | Pattern | Inputs |
|---|---|---|
| headers | CCR | 64870 |
| headers | PPR | 8272 |
| request | CCR | 270686 |
| request | PPR | 68826 |
| request | RRC | 46 |
| request | RRP | 1351 |
| response | CCC! | 13344 |
| response | CCR | 129628 |
| response | PPR | 45589 |
| response | RRC | 6132 |
| response | RRP | 534 |

Disagreements by attributed feature. Each disagreeing input is attributed to the smallest set of features whose removal (a rewrite of just that feature) makes all three parsers agree; an input attributed to two features counts under both, and "alone" counts the inputs where this feature is the whole explanation:

| Feature | Inputs attributed | Alone | Patterns (count) |
|---|---|---|---|
| obs-fold | 258293 | 116039 | CCR 233016, PPR 25277 |
| multi-space-start-line | 104350 | 24524 | CCC! 10136, CCR 85708, PPR 8477, RRC 29 |
| version-digits | 404441 | 219612 | CCR 338185, PPR 66256 |
| high-byte-start-line | 16527 | 11475 | CCC! 3406, CCR 10572, PPR 2431, RRC 118 |
| bare-lf | 213 | 213 | PPR 213 |
| bare-cr | 442 | 434 | PPR 358, RRP 84 |
| leading-empty-lines | 1492 | 1467 | RRC 123, RRP 1369 |
| header-line-without-colon | 1 | 1 | RRP 1 |
| status-code-range | 6495 | 6353 | RRC 6065, RRP 430 |
<!-- END GENERATED: sweep -->

Reading the numbers: the share of disagreeing inputs is large because the mutator produces
version digits, folds and doubled spaces constantly, not because the parsers disagree on
typical traffic. The pinned fact is the set of classes, not the percentage. picohttpparser
and fuzzdiff never differ, which is what the existing harness already guarantees.

## What each class is, which side is stricter, and what the RFC says

**Obsolete line folding (`fold`)**: picohttpparser and fuzzdiff accept and return the
continuation as a header with no name; httparse rejects. RFC 9112 section 5.2: a server that
receives an obs-fold in a request "MUST either reject the message by sending a 400 (Bad
Request) ... or replace each received obs-fold with one or more SP octets prior to
interpreting the field value or forwarding the message downstream." picohttpparser does
neither; it leaves the caller to notice the nameless header. httparse's default (reject) is
one of the two permitted behaviours. For responses, section 5.2 allows a user agent only to
replace, so a strict reject is stricter than a user agent is allowed to be, which is why
httparse has an opt-in for it. fuzzdiff's `reject_obs_fold` is D1.

**Whitespace before the first header line (`ws-first`)**: all three reject. RFC 9112 2.2:
a recipient "MUST either reject the message as invalid or consume each whitespace-preceded
line without further processing of it", and the section names request smuggling as the
reason.

**Number of SP between start-line elements (`sp-count`)**: picohttpparser and fuzzdiff skip
a run of SP; httparse requires exactly one in a request line. RFC 9112 section 3 grammar is a
single SP, and recipients "MAY instead parse on whitespace-delimited word boundaries", with
the warning that "lenient parsing can result in request smuggling security vulnerabilities if
there are multiple recipients of the message and each has its own unique interpretation of
robustness". Both behaviours are permitted. In a status line, httparse keeps the second SP
as the first byte of the reason phrase while picohttpparser drops it (same verdict, different
fields).

**Other whitespace as a separator (`sep`)**: all three reject HTAB; section 3 would permit
accepting it (SP, HTAB, VT, FF, bare CR), so all three are stricter than required.

**HTTP version digits (`version`)**: picohttpparser accepts `HTTP/1.` followed by any
digit; httparse accepts only `HTTP/1.0` and `HTTP/1.1` (its source compares the 8 bytes). RFC
9112 section 2.3: `HTTP-version = HTTP-name "/" DIGIT "." DIGIT`, so `HTTP/1.2` is
grammatical and httparse is stricter than the grammar; `HTTP/2.0`, `HTTP/0.9` and
`HTTP/1.10` are rejected by all three. HTTP-version is case-sensitive, so `http/1.1` is
correctly rejected by all.

**0x80-0xFF in the request-target (`hi-target`)**: picohttpparser and fuzzdiff accept;
httparse rejects. RFC 9112 section 3.2 forms the target from URI grammar (RFC 9110
section 4.1 adopts it from RFC 3986), whose characters are US-ASCII with other octets
percent-encoded (RFC 3986 section 2). A raw high byte is outside the grammar, so httparse is
the conforming side. RFC 9112 section 3.2 also says a recipient "SHOULD NOT attempt to
autocorrect" an invalid request-line, "since the invalid request-line might be deliberately
crafted to bypass security filters along the request chain".

**0x80-0xFF in a field value (`hi-value`)**: all three accept. RFC 9110 section 5.5:
`field-vchar = VCHAR / obs-text`, `obs-text = %x80-FF`. No difference. (In a field name all
three reject, since a name is a token.)

**Field-name characters and whitespace before the colon (`name-charset`, `colon-ws`)**: all
three reject, consistent with the token rule (RFC 9110 sections 5.1 and 5.6.2) and with RFC 9112
section 5.1: a server "MUST reject, with a response status code of 400 (Bad Request), any
received request message that contains whitespace between a header field name and colon",
and a proxy "MUST remove any such whitespace from a response message".

**Bare LF (`bare-lf`)**: all three accept LF alone as a line ending in the start line and
header lines. RFC 9112 section 2.2: "a recipient MAY recognize a single LF as a line terminator
and ignore any preceding CR". **Bare CR (`bare-cr`)**: all three reject; the same section
requires a recipient of a bare CR to "consider that element to be invalid or replace each bare
CR with SP".

**NUL and other control octets (`ctl`)**: all three reject NUL, DEL and other controls in
targets, field values and reason phrases. RFC 9110 section 5.5: a recipient of CR, LF or NUL
within a field value "MUST either reject the message or replace each of those characters with
SP"; other CTLs are invalid but a recipient "MAY retain" them in a safe context, so all three
are stricter than required for those.

**Empty lines before the request (`lead-crlf`)**: httparse skips any number of leading empty
lines; picohttpparser and fuzzdiff skip one. RFC 9112 section 2.2: a server "SHOULD ignore at
least one empty line (CRLF) received prior to the request-line", so both are permitted;
httparse is the more lenient.

**Status-code range (`status`)**: picohttpparser rejects a first digit outside 1-5; httparse
accepts any three digits. RFC 9112 section 4 grammar is `3DIGIT`; RFC 9110 section 15 says "All valid
status codes are within the range of 100 to 599" and that a client receiving an invalid code
"SHOULD process the response as if it had a 5xx (Server Error) status code". So httparse
passes it through, picohttpparser refuses it; neither contradicts a MUST.

**Reason phrase (`reason`)**: both complete. httparse returns an empty reason when the phrase
contains obs-text (its source substitutes an empty string), picohttpparser returns the
bytes. RFC 9112 section 4 allows obs-text in a reason phrase and tells clients to ignore its content.

**Partial input (`timing`)**: no RFC rule; this is implementation behaviour of an incremental
parser. On a prefix that is already invalid, httparse can return an error where
picohttpparser, which wants the whole 9-byte version token and whole line, still says "need more
bytes" (httparse's `parse_version` compares byte by byte to reject early). It also rejects a
fold line before the line is complete. A caller that stops reading at the first verdict will
see different cut-off points. Related: both sides were given 64 header slots, and
picohttpparser rejects at the start of a 65th header line while httparse waits for the line.

**Chunk extensions and chunk-size lines (`chunk-ext`, `chunk-size`)**: RFC 9112 section
7.1 gives `chunk-size = 1*HEXDIG`, and section 7.1.1 gives `chunk-ext-val = token /
quoted-string`; recipients "MUST ignore unrecognized chunk extensions". Observed:

* All three accept bytes in an extension that the grammar does not allow (NUL; 0xFF as a
  token character). httparse's source says it accepts any octet once inside an extension.
* All three reject bare CR in an extension.
* Bare LF inside an extension: picohttpparser and fuzzdiff reject; httparse treats it as part of
  the extension and ends the line at the next CRLF, so the position where chunk data
  starts differs by the length of the text in between. (A unit test pins the consumed
  offset: `5;a=b\nhello\r\n` is a complete 13-byte size line for httparse.)
* An empty size (`\r\n` with no hex digits) is rejected by picohttpparser and fuzzdiff and
  returned as a complete size line (size 0) by `httparse::parse_chunk_size`. That function is a
  helper; a caller can add its own check.
* Both limit chunk-size digits (17 and 18 digits rejected by all three).

## Which classes matter for request-smuggling research

Request smuggling, as RFC 9112 section 11.2 puts it, "exploits differences in protocol
parsing among various recipients to hide additional requests". The general phenomenon is a
parser differential: two components in a chain read the same bytes and draw different
conclusions about header fields or message boundaries. A class matters for that study when the
two parsers can disagree about those things, not merely about accept versus reject. Using the
RFC's own statements about why a rule exists:

* **Directly tied to header or framing interpretation, and named by the RFC**: obsolete line
  folding (section 5.2), whitespace before the first header line (section 2.2, which cites
  request smuggling), whitespace between name and colon (section 5.1, which cites routing and
  response-handling differences), lenient whitespace in the request line (section 3, which
  cites smuggling when robustness differs across recipients), bare CR and bare LF handling
  (section 2.2), and NUL in field values (RFC 9110 section 5.5, "invalid and dangerous").
* **Framing-adjacent**: where a chunk-size line ends (bare LF inside an extension, empty
  size), because the chunk size and the extension boundary decide where data starts.
* **Not a smuggling concern on its own**: version digits, status-code range, reason phrase
  and 0x80 in a field value. They change accept or reject, or the fields shown, but do not
  by themselves change how a message is split. A high byte in the request-target is a
  filter-bypass concern more than a framing one.
* **Both agree, but the caller decides**: the first rows of the curated table show that all
  three parsers accept a request with both `Content-Length` and `Transfer-Encoding`, with two
  different `Content-Length` values, and with `Transfer-Encoding: xchunked`. A header-block
  parser reports them; RFC 9112 sections 6.1 to 6.3 say what a recipient must do with them,
  and that is above this layer.

In short, picohttpparser (and so fuzzdiff by default) is the more lenient
parser in most of the classes above with RFC 9112 text behind them (fold, SP runs, high bytes
in the target, empty chunk size); the exception is bare LF inside a chunk extension, where
httparse is the lenient one. An intermediary built on one and a
server built on the other would see different messages on those inputs. Nothing in this
repository tests such a pair.

## What the CI test pins

`harness/tests/httparse_classes.rs` fails when:

* any curated example's outcome pattern changes (a class disappears, or a new disagreement
  appears in a class that was agreeing);
* the generated curated table in this file is out of date;
* a deterministic 50,000-input sweep finds an input where picohttpparser and fuzzdiff differ,
  an input no known feature explains, a disagreement attributed to a feature outside the
  known set, or an (entry point, pattern) pair outside the known set;
* httparse's chunk-line handling of bare LF in an extension changes.

Bumping httparse is therefore a deliberate act: the test shows what moved, and this page is
updated from the new output.

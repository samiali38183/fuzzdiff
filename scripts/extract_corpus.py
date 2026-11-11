"""Extract the inputs from picohttpparser's own test.c into corpus/ files.

Usage: python scripts/extract_corpus.py path/to/picohttpparser/test.c

Only the input byte strings are taken (the expected results are not: the
harness compares against the C library itself). test.c is MIT licensed, see
c/LICENSE. Hand-written cases live next to the extracted ones in corpus/.
"""
import pathlib
import re
import sys

LIT = re.compile(r'"((?:[^"\\]|\\.)*)"')


def c_unescape(s: str) -> bytes:
    out = bytearray()
    i = 0
    simple = {"n": 10, "r": 13, "t": 9, "\\": 92, '"': 34, "'": 39, "a": 7, "b": 8, "f": 12, "v": 11}
    while i < len(s):
        ch = s[i]
        if ch != "\\":
            out += ch.encode("utf-8")
            i += 1
            continue
        i += 1
        ch = s[i]
        if ch in simple:
            out.append(simple[ch])
            i += 1
        elif ch == "x":
            j = i + 1
            while j < len(s) and s[j] in "0123456789abcdefABCDEF":
                j += 1
            out.append(int(s[i + 1 : j], 16) & 0xFF)
            i = j
        elif ch in "01234567":
            j = i
            while j < len(s) and j < i + 3 and s[j] in "01234567":
                j += 1
            out.append(int(s[i:j], 8) & 0xFF)
            i = j
        else:
            raise ValueError(f"unhandled escape \\{ch}")
    return bytes(out)


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def main() -> None:
    src = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
    root = pathlib.Path(__file__).resolve().parent.parent / "corpus"
    section = None
    counts: dict[str, int] = {}
    names: dict[str, set] = {}

    def emit(kind: str, name: str, data: bytes) -> None:
        d = root / kind
        d.mkdir(parents=True, exist_ok=True)
        base = f"picohttpparser-{slug(name) or 'case'}"
        n = base
        k = 2
        while n in names.setdefault(kind, set()):
            n = f"{base}-{k}"
            k += 1
        names[kind].add(n)
        (d / f"{n}.bin").write_bytes(data)
        counts[kind] = counts.get(kind, 0) + 1

    for line in src:
        m = re.match(r"static void (test_\w+)\(", line)
        if m:
            section = m.group(1)
        stripped = line.strip()
        if section in ("test_request", "test_response", "test_headers") and stripped.startswith("PARSE(\""):
            lits = LIT.findall(stripped)
            kind = {"test_request": "request", "test_response": "response", "test_headers": "headers"}[section]
            comment = lits[-1] if len(lits) > 1 else "case"
            emit(kind, comment, c_unescape(lits[0]))
        elif section in ("test_chunked", "test_chunked_consume_trailer", "test_chunked_leftdata"):
            if "chunked_test_runners[i](" in stripped or "test_chunked_failure(" in stripped:
                lits = LIT.findall(stripped)
                if lits:
                    tag = "trailer-" if section == "test_chunked_consume_trailer" else ""
                    emit("chunked", tag + lits[0][:24], c_unescape(lits[0]))
    for kind, n in sorted(counts.items()):
        print(f"{kind}: {n} files")


if __name__ == "__main__":
    main()

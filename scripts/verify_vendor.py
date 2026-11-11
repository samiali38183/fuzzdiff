"""Check that the vendored C files match the hashes recorded in c/VENDORED.md."""
import hashlib
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
notes = (root / "c" / "VENDORED.md").read_text(encoding="utf-8")
ok = True
for name in ("picohttpparser.c", "picohttpparser.h"):
    want = re.search(rf"SHA-256 {re.escape(name)}: ([0-9a-f]{{64}})", notes).group(1)
    got = hashlib.sha256((root / "c" / name).read_bytes()).hexdigest()
    print(f"{name}: {'ok' if got == want else 'MISMATCH'}")
    ok &= got == want
sys.exit(0 if ok else 1)

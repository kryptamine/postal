#!/usr/bin/env python3
"""The HTML mails a rendering check runs over: the .eml fixtures, then every
body in a local cache. Run as a script, it writes each to <out>/<name>.html.
"""

import email
import email.policy
import os
import pathlib
import sqlite3
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def default_cache():
    base = os.environ.get("XDG_CACHE_HOME") or pathlib.Path.home() / ".cache"
    return pathlib.Path(base) / "postal" / "cache.sqlite3"


def html_part(raw):
    """The HTML body and the subject, or None for a mail with no HTML."""
    try:
        msg = email.message_from_bytes(raw, policy=email.policy.default)
        part = msg.get_body(preferencelist=("html",))
        if part is None:
            return None
        subject = " ".join(str(msg.get("subject") or "").split())
        return part.get_content(), subject
    # Mail is hostile input: any parse failure only drops that mail.
    except Exception:
        return None


def raws(cache):
    for path in sorted((ROOT / "tests" / "fixtures").glob("*.eml")):
        yield "fixture", path.name, path.read_bytes()
    if not cache.is_file():
        return
    try:
        db = sqlite3.connect(f"file:{cache}?mode=ro", uri=True)
        try:
            for key, raw in db.execute("select key, raw from body"):
                yield "cached", str(key), raw
        finally:
            db.close()
    except sqlite3.Error as err:
        print(f"warning: could not read cache {cache}: {err}", file=sys.stderr)


def mails(cache):
    """(origin, name, subject, html) for each HTML mail, fixtures first."""
    for origin, name, raw in raws(cache):
        found = html_part(raw)
        if found is not None:
            html, subject = found
            yield origin, name, subject, html


def main():
    cache = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1] else default_cache()
    out = pathlib.Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    for _, name, _, html in mails(cache):
        (out / f"{name}.html").write_text(html)


if __name__ == "__main__":
    main()

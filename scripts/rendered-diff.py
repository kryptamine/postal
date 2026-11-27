#!/usr/bin/env python3
"""Diffs laid-out HTML mails between a baseline ref and the working tree.

Runs each binary's layout process over the corpus.py mails and compares the
grids byte for byte; exits 1 when any differs, as diff does.
"""

import argparse
from concurrent.futures import ThreadPoolExecutor
import dataclasses
import difflib
import os
import pathlib
import shutil
import subprocess
import sys

import corpus

ROOT = corpus.ROOT
OUT = ROOT / "target" / "rendered-diff"
DEFAULT_COLS = "40,74,120"
# The layout process's own limit; past it the engine is taken to hang.
TIMEOUT_SECS = 5
BUILD_INPUTS = ("Cargo.toml", "Cargo.lock", "build.rs", "src", "cpp", "vendor")


@dataclasses.dataclass
class Change:
    cols: int
    summary: str
    before: list
    after: list


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def build_candidate():
    subprocess.run(["cargo", "build", "--release"], cwd=ROOT, check=True)
    return ROOT / "target" / "release" / "postal"


def build_baseline(ref, candidate):
    sha = git("rev-parse", "--short=12", ref)
    cached = OUT / f"base-{sha}"
    if cached.is_file():
        return sha, cached

    # Untracked files count: build.rs compiles every file in cpp/.
    dirty = git("status", "--porcelain", "--untracked-files=all", "--", *BUILD_INPUTS)
    if sha == git("rev-parse", "--short=12", "HEAD") and not dirty:
        return sha, candidate

    OUT.mkdir(parents=True, exist_ok=True)
    src_dir = OUT / f"src-{sha}"
    shutil.rmtree(src_dir, ignore_errors=True)
    src_dir.mkdir()

    # Export via git archive so building a baseline never writes to .git.
    archive = subprocess.Popen(["git", "archive", sha], cwd=ROOT, stdout=subprocess.PIPE)
    try:
        subprocess.run(["tar", "-x", "-C", str(src_dir)], stdin=archive.stdout, check=True)
        archive.stdout.close()
        if archive.wait() != 0:
            raise RuntimeError(f"git archive {sha} failed")

        target_dir = OUT / "target"
        env = {**os.environ, "CARGO_TARGET_DIR": str(target_dir)}
        subprocess.run(["cargo", "build", "--release"], cwd=src_dir, env=env, check=True)
        shutil.copy2(target_dir / "release" / "postal", cached)
    finally:
        shutil.rmtree(src_dir, ignore_errors=True)

    return sha, cached


def lay_out(binary, cols, html, *mode):
    """The layout process's stdout, or None when it failed or hung."""
    try:
        proc = subprocess.run(
            [str(binary), "__lay-out", *mode, str(cols)],
            input=html,
            capture_output=True,
            timeout=TIMEOUT_SECS,
        )
    except subprocess.TimeoutExpired:
        return None
    return proc.stdout if proc.returncode == 0 else None


def compare(base_bin, cand_bin, cols, html):
    """None when both lay `html` out alike, else the Change."""
    grids = [lay_out(b, cols, html) for b in (base_bin, cand_bin)]
    if grids[0] == grids[1]:
        return None
    if None in grids:
        return Change(cols, "failed", [], [])

    # A baseline older than the text mode still compares, by bytes alone.
    texts = [lay_out(b, cols, html, "text") for b in (base_bin, cand_bin)]
    if None in texts:
        return Change(cols, "bytes", [], [])

    before, after = (t.decode().splitlines() for t in texts)
    notes = []
    if before[0] != after[0]:
        notes.append(f"{before[0]}->{after[0]}")
    if len(before) != len(after):
        notes.append(f"lines:{len(before)}->{len(after)}")
    notes.append("text" if before != after else "style")
    return Change(cols, " ".join(notes), before, after)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", default="main", help="baseline git ref (default: main)")
    parser.add_argument("--baseline", type=pathlib.Path, help="pre-built baseline binary")
    parser.add_argument("--candidate", type=pathlib.Path, help="pre-built candidate binary")
    parser.add_argument("--cols", default=DEFAULT_COLS, help=f"widths (default: {DEFAULT_COLS})")
    parser.add_argument("--cache", type=pathlib.Path, default=corpus.default_cache())
    parser.add_argument("--diff", action="store_true", help="print a text diff per change")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    args = parser.parse_args()

    widths = [int(c) for c in args.cols.replace(",", " ").split()]
    cand_bin = args.candidate or build_candidate()
    if args.baseline:
        base_bin, base_label = args.baseline, str(args.baseline)
    else:
        sha, base_bin = build_baseline(args.base, cand_bin)
        base_label = f"{args.base} ({sha})"

    mails = list(corpus.mails(args.cache))
    fixtures = sum(origin == "fixture" for origin, *_ in mails)
    print(f"baseline: {base_label}")
    print(f"corpus: {fixtures} fixtures, {len(mails) - fixtures} cached HTML mails")

    changes = {}
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for cols in widths:
            found = pool.map(lambda m: compare(base_bin, cand_bin, cols, m[3].encode()), mails)
            changed = 0
            for (_, name, subject, _), change in zip(mails, found):
                if change is not None:
                    changed += 1
                    changes.setdefault((name, subject), []).append(change)
            print(f"cols {cols}: {changed} of {len(mails)} changed")

    print(f"total: {len(changes)} of {len(mails)} mails changed")
    for (name, subject), found in changes.items():
        details = ", ".join(f"{c.cols}c [{c.summary}]" for c in found)
        print(f"  {name}{f' — {subject[:60]}' if subject else ''}: {details}")
        if not args.diff:
            continue
        for c in found:
            diff = difflib.unified_diff(
                c.before,
                c.after,
                fromfile=f"{name}@{c.cols} (base)",
                tofile=f"{name}@{c.cols} (head)",
                lineterm="",
            )
            for line in diff:
                print(f"    {line}")

    sys.exit(1 if changes else 0)


if __name__ == "__main__":
    main()

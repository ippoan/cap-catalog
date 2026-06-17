#!/usr/bin/env python3
"""Rust extractor for cap-catalog (Refs #4).

Reads rustdoc JSON output (`cargo +nightly rustdoc --output-format json -Z unstable-options`)
and emits catalog-extract.jsonl on stdout — 1 line per public item.

JSONL schema (matches schema/catalog.sql.symbols + features):

    {
      "repo": "ippoan/auth-worker",
      "language": "rust",
      "kind": "fn" | "struct" | "trait" | ...,
      "name": "createAuthFetch",
      "fq_path": "auth_client::createAuthFetch",
      "signature": ... (best-effort),
      "doc": "...",
      "file": "src/lib.rs",
      "line": 42,
      "commit_sha": "abc123",
      "features": ["auth-fetch", ...]
    }

`@feature: <name>` 注釈は doc-comment 内に「`@feature: foo, bar` または `@feature: foo`
が 1 行に書いてある」前提で正規表現 (case-insensitive) で集める。

Usage:
    python3 extract-rust.py --repo ippoan/auth-worker --sha abc123 --json-dir target/doc > catalog-extract.jsonl

Multiple `*.json` files in `--json-dir` (workspace の各 crate 分) は順に処理する。
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from typing import Iterable, Iterator

FEATURE_RE = re.compile(r"@feature:\s*([^\n\r]+)", re.IGNORECASE)

# rustdoc Item kind を symbols.kind に揃える alias 表。
KIND_ALIASES: dict[str, str] = {
    "function": "fn",
    "method": "fn",
    "struct": "struct",
    "enum": "enum",
    "trait": "trait",
    "module": "module",
    "type_alias": "type",
    "typedef": "type",
    "constant": "const",
    "static": "static",
    "macro": "macro",
    "proc_macro": "macro",
    "union": "union",
    "trait_alias": "trait",
}

# 「カタログに載せる」kind の white-list (alias 後の値)。これに無い kind の item
# はカタログに乗らない:
#   - struct_field / variant / assoc_const / assoc_type — 親 item に従属する内部要素
#   - impl / primitive / extern_crate — symbol search の対象外
#   - derive macro が展開した synthetic method (rustdoc は `inner.function` で出す
#     が、`paths` に entry を持たないため後段の fq_path フィルタで落ちる)
KIND_ALLOWED: frozenset[str] = frozenset(
    {"fn", "struct", "enum", "trait", "module", "type", "const", "static", "macro", "union"}
)


def map_kind(rustdoc_kind: str) -> str:
    """Map rustdoc Item kind → cap-catalog symbols.kind."""
    return KIND_ALIASES.get(rustdoc_kind, rustdoc_kind)


def extract_features(docs: str | None) -> list[str]:
    if not docs:
        return []
    feats: list[str] = []
    for m in FEATURE_RE.finditer(docs):
        for raw in m.group(1).split(","):
            tag = raw.strip().rstrip(".").strip()
            if tag and tag not in feats:
                feats.append(tag)
    return feats


def iter_items(rustdoc: dict) -> Iterator[dict]:
    """Yield normalized item dicts from one rustdoc JSON document."""
    index = rustdoc.get("index", {})
    paths = rustdoc.get("paths", {})

    for item_id, item in index.items():
        # skip non-local (= dep) items: rustdoc emits them in index but with crate_id != 0
        if item.get("crate_id", 0) != 0:
            continue
        # need a name
        name = item.get("name")
        if not name:
            continue
        # visibility — keep public only (private items are noise in a feature catalog)
        vis = item.get("visibility")
        if vis not in ("public", "default"):
            # `default` covers trait items / enum variants whose visibility inherits container
            continue
        # rustdoc 'kind' lives in `inner` as a single-key dict in newer formats; older
        # formats put it at top-level under `kind`. Probe both for compatibility.
        kind_raw = item.get("kind")
        if not kind_raw:
            inner = item.get("inner")
            if isinstance(inner, dict) and inner:
                kind_raw = next(iter(inner.keys()))
            else:
                kind_raw = "unknown"
        kind = map_kind(kind_raw)
        if kind not in KIND_ALLOWED:
            # struct_field / variant / assoc_const / assoc_type / impl 等を弾く
            continue

        # fq_path は **rustdoc paths テーブルに登録された item に限る**。
        # `paths` は「外から呼べる top-level item」のみを含むため、impl 内の
        # method や derive macro 展開で生成された synthetic 関数は登録されない
        # → ここで自然に落ちる (= catalog から noise が消える)。
        path_entry = paths.get(item_id)
        if not path_entry:
            continue
        path_segments = path_entry.get("path") or []
        if not path_segments:
            continue
        fq_path = "::".join(path_segments)

        # span
        span = item.get("span") or {}
        file = span.get("filename")
        begin = span.get("begin") or [None, None]
        line = begin[0] if isinstance(begin, list) and begin else None

        docs = item.get("docs")
        features = extract_features(docs)

        yield {
            "kind": kind,
            "name": name,
            "fq_path": fq_path,
            "doc": docs,
            "file": file,
            "line": line,
            "features": features,
        }


def emit_lines(
    repo: str,
    sha: str | None,
    rustdoc_files: Iterable[pathlib.Path],
    out,
) -> int:
    seen: set[str] = set()
    n = 0
    for rd_path in rustdoc_files:
        try:
            data = json.loads(rd_path.read_text())
        except Exception as e:
            print(f"::warning::failed to parse {rd_path}: {e}", file=sys.stderr)
            continue
        for item in iter_items(data):
            key = f"{repo}|rust|{item['fq_path']}"
            if key in seen:
                continue
            seen.add(key)
            obj = {
                "repo": repo,
                "language": "rust",
                "kind": item["kind"],
                "name": item["name"],
                "fq_path": item["fq_path"],
                "signature": None,  # 後続 PR で sig 抽出予定
                "doc": item["doc"],
                "file": item["file"],
                "line": item["line"],
                "commit_sha": sha,
                "features": item["features"],
            }
            out.write(json.dumps(obj, ensure_ascii=False, separators=(",", ":")))
            out.write("\n")
            n += 1
    return n


def main() -> int:
    p = argparse.ArgumentParser(description="Rust extractor for cap-catalog")
    p.add_argument("--repo", required=True, help="`owner/name` of the source repo (= symbols.repo)")
    p.add_argument("--sha", default="", help="commit sha at extract time (= symbols.commit_sha)")
    p.add_argument(
        "--json-dir",
        type=pathlib.Path,
        required=True,
        help="Directory containing rustdoc *.json (typically target/doc)",
    )
    p.add_argument("--out", type=argparse.FileType("w"), default=sys.stdout)
    args = p.parse_args()

    json_files = sorted(args.json_dir.glob("*.json"))
    if not json_files:
        print(f"::warning::no *.json in {args.json_dir}; emitting empty JSONL", file=sys.stderr)
        return 0

    n = emit_lines(args.repo, args.sha or None, json_files, args.out)
    print(f"emitted {n} symbol(s) from {len(json_files)} rustdoc file(s)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

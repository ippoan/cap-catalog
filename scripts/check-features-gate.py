#!/usr/bin/env python3
"""Doc-comment feature gate for cap-catalog (Refs #24).

`crates/**/*.rs` を走査し、`pub <kind> <name>` 形式の宣言ごとに **直前の
`///` doc-comment block 内に `@feature:` がある**ことを CI で gate する。
無ければ `::error` (= GitHub Annotations) を吐いて exit 1 = loud fail。

crate root (lib.rs / main.rs) は **ファイル先頭の `//!` block 内** に
`@feature:` があることも検査する (= crate そのものが何の能力か明示する)。

検出される pub item:
    pub fn / pub struct / pub enum / pub trait / pub const / pub static
    pub union / pub type / pub macro

明示的に skip するもの (= 自身は能力を持たない、再 export / 構造のみ):
    pub use ...          # re-export
    pub mod <name>;      # module 宣言 (本体は別ファイル、そっちで gate される)
    pub fn main(...)     # binary entrypoint (crate 自体の @feature: で表現)

Usage:
    python3 scripts/check-features-gate.py
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

PUB_ITEM_RE = re.compile(
    r"^\s*pub\s+(?:fn|struct|enum|trait|const|static|union|type|macro)\s+(\w+)",
)
PUB_USE_RE = re.compile(r"^\s*pub\s+use\s")
PUB_MOD_DECL_RE = re.compile(r"^\s*pub\s+mod\s+\w+\s*;\s*$")
PUB_FN_MAIN_RE = re.compile(r"^\s*pub\s+fn\s+main\s*\(")
FEATURE_RE = re.compile(r"@feature:", re.IGNORECASE)


def has_feature_in_doc_block(lines: list[str], item_idx: int) -> bool:
    """item_idx の **直前** から連続する `///` 行に `@feature:` があるか。

    `#[...]` attribute は doc-block の一部とみなして読み飛ばす (= macro derive 等で
    doc-comment と pub 宣言の間に attribute が挟まることがある)。
    """
    j = item_idx - 1
    while j >= 0:
        s = lines[j].strip()
        if s.startswith("///"):
            if FEATURE_RE.search(s):
                return True
            j -= 1
        elif s.startswith("#[") or s.endswith("]"):
            # attribute (single line or multi-line). skip and keep looking.
            j -= 1
        elif s == "":
            # 空行は doc-block を切る (= rustdoc も切る)
            return False
        else:
            return False
    return False


def has_inner_feature_in_crate_root(lines: list[str]) -> bool:
    """ファイル先頭の `//!` block (= crate root inner doc-comment) を見て
    `@feature:` があるか確認する。最初の非コメント・非空行で打ち切る。
    """
    for line in lines:
        s = line.strip()
        if s.startswith("//!"):
            if FEATURE_RE.search(s):
                return True
        elif s == "" or s.startswith("#!["):
            # 空行 / inner attribute (e.g. #![forbid(unsafe_code)]) は許容して継続
            continue
        else:
            # 最初の実コード行に到達 = inner doc-block 終了
            break
    return False


def crate_root_line(lines: list[str]) -> int:
    """`::error file=,line=` のために、crate root 報告用の行番号を返す。
    内側 doc-comment が無いなら 1。あるなら最初の非 //! 行。
    """
    for i, line in enumerate(lines, start=1):
        s = line.strip()
        if s == "" or s.startswith("//!") or s.startswith("#!["):
            continue
        return i
    return 1


def scan_file(path: pathlib.Path) -> list[tuple[int, str]]:
    """ファイル 1 つを scan し、`@feature:` 欠落の (行番号, 行内容) を返す。"""
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    misses: list[tuple[int, str]] = []

    # crate root: lib.rs / main.rs だけが対象
    if path.name in {"lib.rs", "main.rs"}:
        if not has_inner_feature_in_crate_root(lines):
            misses.append((crate_root_line(lines), "crate root (no //! @feature:)"))

    # pub item を 1 行ずつ
    for i, line in enumerate(lines):
        if PUB_USE_RE.match(line) or PUB_MOD_DECL_RE.match(line) or PUB_FN_MAIN_RE.match(line):
            continue
        if not PUB_ITEM_RE.match(line):
            continue
        if not has_feature_in_doc_block(lines, i):
            misses.append((i + 1, line.strip()[:120]))

    return misses


def main() -> int:
    crates_dir = ROOT / "crates"
    if not crates_dir.is_dir():
        print(f"::warning::{crates_dir} not found; nothing to gate.", file=sys.stderr)
        return 0

    files = sorted(crates_dir.rglob("*.rs"))
    total_misses = 0
    for p in files:
        misses = scan_file(p)
        for ln, snippet in misses:
            rel = p.relative_to(ROOT)
            print(
                f"::error file={rel},line={ln}::pub item に @feature: 注釈がありません: {snippet}",
                file=sys.stderr,
            )
            total_misses += 1

    if total_misses:
        print("", file=sys.stderr)
        print(
            f"❌ {total_misses} 件の pub item に @feature: 注釈が不足しています。",
            file=sys.stderr,
        )
        print(
            "→ ///<newline>/// @feature: <capability-name> を直前に追加してください。",
            file=sys.stderr,
        )
        print(
            "→ crate root (lib.rs/main.rs) は //!<newline>//! @feature: ... を先頭 block に。",
            file=sys.stderr,
        )
        print(
            "→ 設計: Refs ippoan/cap-catalog#24 (doc-comment gate, Phase C dogfood)",
            file=sys.stderr,
        )
        return 1

    print(f"OK: {len(files)} files scanned, all pub items have @feature:.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

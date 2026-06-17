"""Unit tests for scripts/extract-rust.py.

rustdoc JSON の minimal な fixture を 1 つ作って:
  - public function → kind=fn / fq_path / line / docs / features
  - private item → skip
  - dep crate (crate_id != 0) → skip
  - @feature: 抽出 (1 行内複数 / case-insensitive)

を gate する。実 rustdoc 出力に依存しない (= CI で rustc nightly 不要)。
"""

from __future__ import annotations

import importlib.util
import io
import json
import pathlib
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "extract-rust.py"

spec = importlib.util.spec_from_file_location("extract_rust", SCRIPT)
assert spec is not None and spec.loader is not None
extract_rust = importlib.util.module_from_spec(spec)
sys.modules["extract_rust"] = extract_rust
spec.loader.exec_module(extract_rust)


def make_fixture() -> dict:
    """Minimal rustdoc-shaped document with 4 items."""
    return {
        "root": "0:0",
        "crate_version": "0.1.0",
        "format_version": 30,
        "paths": {
            "0:0": {"crate_id": 0, "path": ["my_crate"], "kind": "module"},
            "0:1": {"crate_id": 0, "path": ["my_crate", "foo"], "kind": "function"},
            "0:2": {"crate_id": 0, "path": ["my_crate", "Bar"], "kind": "struct"},
            "0:3": {"crate_id": 0, "path": ["my_crate", "private_helper"], "kind": "function"},
            "0:4": {"crate_id": 1, "path": ["other_crate", "external"], "kind": "function"},
        },
        "index": {
            "0:1": {
                "crate_id": 0,
                "name": "foo",
                "visibility": "public",
                "docs": "Public fn.\n\n@feature: alpha, beta\n",
                "span": {"filename": "src/lib.rs", "begin": [10, 1], "end": [12, 2]},
                "inner": {"function": {}},
            },
            "0:2": {
                "crate_id": 0,
                "name": "Bar",
                "visibility": "public",
                "docs": "Public struct.\n@FEATURE: gamma\n",
                "span": {"filename": "src/lib.rs", "begin": [30, 1], "end": [35, 2]},
                "inner": {"struct": {}},
            },
            "0:3": {
                "crate_id": 0,
                "name": "private_helper",
                "visibility": "crate",
                "docs": "Private.",
                "span": {"filename": "src/lib.rs", "begin": [50, 1], "end": [52, 2]},
                "inner": {"function": {}},
            },
            "0:4": {
                "crate_id": 1,
                "name": "external",
                "visibility": "public",
                "docs": "From dep.",
                "span": {"filename": "ext/lib.rs", "begin": [1, 1], "end": [2, 2]},
                "inner": {"function": {}},
            },
        },
    }


def test_extract_features_one_line_csv_case_insensitive():
    assert extract_rust.extract_features("@feature: a, b , c") == ["a", "b", "c"]
    assert extract_rust.extract_features("@FEATURE: foo") == ["foo"]
    assert extract_rust.extract_features("@Feature: x.\n@feature: x, y") == ["x", "y"]
    assert extract_rust.extract_features(None) == []
    assert extract_rust.extract_features("no tag here") == []


def test_map_kind():
    assert extract_rust.map_kind("function") == "fn"
    assert extract_rust.map_kind("method") == "fn"
    assert extract_rust.map_kind("type_alias") == "type"
    assert extract_rust.map_kind("trait") == "trait"
    # passthrough for unknown
    assert extract_rust.map_kind("weird") == "weird"


def test_iter_items_filters_and_normalizes():
    items = list(extract_rust.iter_items(make_fixture()))
    names = [i["name"] for i in items]
    # private_helper (visibility=crate) と external (crate_id!=0) は弾く
    assert names == ["foo", "Bar"]
    foo = items[0]
    assert foo["kind"] == "fn"
    assert foo["fq_path"] == "my_crate::foo"
    assert foo["file"] == "src/lib.rs"
    assert foo["line"] == 10
    assert foo["features"] == ["alpha", "beta"]
    bar = items[1]
    assert bar["kind"] == "struct"
    assert bar["features"] == ["gamma"]


def test_emit_lines_writes_jsonl_with_required_fields():
    fixture = make_fixture()
    with tempfile.TemporaryDirectory() as td:
        d = pathlib.Path(td)
        (d / "my_crate.json").write_text(json.dumps(fixture))
        out = io.StringIO()
        n = extract_rust.emit_lines("ippoan/test", "deadbeef", [d / "my_crate.json"], out)
    assert n == 2
    lines = [json.loads(l) for l in out.getvalue().splitlines()]
    required = {"repo", "language", "kind", "name", "fq_path"}
    for obj in lines:
        missing = required - obj.keys()
        assert not missing, f"missing fields: {missing}"
        assert obj["language"] == "rust"
        assert obj["repo"] == "ippoan/test"
        assert obj["commit_sha"] == "deadbeef"
    assert {l["name"] for l in lines} == {"foo", "Bar"}


def test_emit_lines_dedupes_across_files():
    """Same fq_path appearing in 2 rustdoc files = emit once (workspace 重複保護)."""
    fixture = make_fixture()
    with tempfile.TemporaryDirectory() as td:
        d = pathlib.Path(td)
        (d / "a.json").write_text(json.dumps(fixture))
        (d / "b.json").write_text(json.dumps(fixture))
        out = io.StringIO()
        n = extract_rust.emit_lines("r", "s", [d / "a.json", d / "b.json"], out)
    assert n == 2  # dedupe: foo + Bar, not 4


if __name__ == "__main__":
    # 依存ゼロで動かす (CI で pytest 未導入を回避): 直接 test_* を呼ぶ。
    fails = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            try:
                fn()
                print(f"PASS {name}")
            except AssertionError as e:
                fails += 1
                print(f"FAIL {name}: {e}", file=sys.stderr)
            except Exception as e:
                fails += 1
                print(f"ERROR {name}: {e}", file=sys.stderr)
    if fails:
        print(f"{fails} test(s) failed", file=sys.stderr)
        sys.exit(1)
    print("all extract-rust tests passed")
